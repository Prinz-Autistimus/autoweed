//! Regelschleife: läuft in einem eigenen Thread, liest die Sensoren, entscheidet
//! über Heizung, Licht und Pumpe und schreibt Status und Messwerte in [`Shared`].

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use autoweed_shared::{
    Actuator, ActuatorStates, BoardConfig, BoardError, BoardStatus, BufferEntry, ErrorKind, Event,
    EventKind, MAX_MANUAL_PUMP_SECS, MAX_OVERRIDE_MINUTES, Measurement, Override, Overrides,
    PumpReason, SensorValues,
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::hardware::{Climate, Hardware};
use crate::light;
use crate::state::{Command, Shared, Storage};

const TICK: Duration = Duration::from_secs(1);
/// Wartezeiten zwischen zwei Neuinitialisierungen eines ausgefallenen Sensors.
const BACKOFF_SECS: [i64; 6] = [1, 2, 5, 10, 30, 60];
const PUMP_STATE_FILE: &str = "pump.json";

/// Wird pro Gießvorgang gespeichert, damit Tageslimit und Sperre einen
/// Neustart überstehen.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PumpState {
    day: Option<NaiveDate>,
    pumped_secs_today: u32,
    last_dose_at: Option<DateTime<Utc>>,
    /// Bodenfeuchte vor dem letzten automatischen Gießvorgang.
    soil_before_last_dose: Option<f32>,
    ineffective_doses: u32,
    locked: bool,
}

/// Ein Sensor mit Back-off nach Fehlern.
struct SensorSlot<T> {
    value: Option<T>,
    failing: bool,
    attempt: usize,
    next_retry: DateTime<Utc>,
}

impl<T: Copy> SensorSlot<T> {
    fn new() -> Self {
        Self {
            value: None,
            failing: false,
            attempt: 0,
            next_retry: DateTime::<Utc>::MIN_UTC,
        }
    }

    /// Liest den Sensor, wenn eine Messung fällig ist bzw. der nächste
    /// Wiederholungsversuch ansteht. Gibt `Some(Err)` beim ersten Fehler
    /// und `Some(Ok)` bei der Erholung zurück, damit der Aufrufer Ereignisse erzeugen kann.
    fn poll(
        &mut self,
        now: DateTime<Utc>,
        measure_due: bool,
        hw: &mut dyn Hardware,
        reinit: fn(&mut dyn Hardware) -> anyhow::Result<()>,
        read: fn(&mut dyn Hardware) -> anyhow::Result<T>,
    ) -> Option<Result<(), String>> {
        if self.failing {
            if now < self.next_retry {
                return None;
            }
        } else if !measure_due {
            return None;
        }
        let was_failing = self.failing;
        let result = if was_failing {
            reinit(hw).and_then(|()| read(hw))
        } else {
            read(hw)
        };
        match result {
            Ok(value) => {
                self.value = Some(value);
                self.failing = false;
                self.attempt = 0;
                was_failing.then_some(Ok(()))
            }
            Err(err) => {
                self.value = None;
                self.failing = true;
                let delay = BACKOFF_SECS[self.attempt.min(BACKOFF_SECS.len() - 1)];
                self.attempt += 1;
                self.next_retry = now + TimeDelta::seconds(delay);
                tracing::warn!("Sensorfehler (nächster Versuch in {delay} s): {err:#}");
                (!was_failing).then(|| Err(format!("{err:#}")))
            }
        }
    }
}

pub struct Controller {
    hardware: Box<dyn Hardware>,
    shared: Arc<Mutex<Shared>>,
    storage: Storage,
    time_synced: Box<dyn Fn() -> bool + Send>,
    climate: SensorSlot<Climate>,
    soil_raw: SensorSlot<i16>,
    next_measure: DateTime<Utc>,
    actuators: ActuatorStates,
    overrides: Overrides,
    pump_until: Option<DateTime<Utc>>,
    pump: PumpState,
    errors: BTreeMap<ErrorKindKey, BoardError>,
    events: Vec<Event>,
}

/// `ErrorKind` als sortierbarer Schlüssel (feste Reihenfolge in der UI).
type ErrorKindKey = u8;

fn error_key(kind: ErrorKind) -> ErrorKindKey {
    kind as u8
}

impl Controller {
    pub fn new(
        hardware: Box<dyn Hardware>,
        shared: Arc<Mutex<Shared>>,
        storage: Storage,
        time_synced: Box<dyn Fn() -> bool + Send>,
    ) -> Self {
        let pump = storage.load(PUMP_STATE_FILE).unwrap_or_default();
        Self {
            hardware,
            shared,
            storage,
            time_synced,
            climate: SensorSlot::new(),
            soil_raw: SensorSlot::new(),
            next_measure: DateTime::<Utc>::MIN_UTC,
            actuators: ActuatorStates::default(),
            overrides: Overrides::default(),
            pump_until: None,
            pump,
            errors: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    /// Läuft, bis `stop` gesetzt wird. Danach wird die Pumpe abgeschaltet;
    /// Heizung und Licht behalten ihren Zustand bis zum nächsten Start.
    pub fn run(mut self, stop: Arc<AtomicBool>) {
        // Beim Start ist alles aus, bis der erste Durchlauf entscheidet.
        for actuator in [Actuator::Heater, Actuator::Light, Actuator::Pump] {
            self.switch(actuator, false, true);
        }
        if self.pump.locked {
            self.raise(
                ErrorKind::BucketEmpty,
                "Pumpe gesperrt: Eimer leer? In der WebUI entsperren.",
            );
        }
        while !stop.load(Ordering::Relaxed) {
            self.tick(Utc::now());
            std::thread::sleep(TICK);
        }
        self.switch(Actuator::Pump, false, true);
        tracing::info!("Regelschleife beendet, Pumpe aus");
    }

    fn tick(&mut self, now: DateTime<Utc>) {
        let (config, has_config, commands) = {
            let mut shared = self.shared.lock().unwrap();
            (
                shared.config.clone(),
                shared.config_from_ecs,
                std::mem::take(&mut shared.commands),
            )
        };

        for command in commands {
            self.handle(command, now);
        }
        self.expire_overrides(now);

        let measure_due = now >= self.next_measure;
        if measure_due {
            self.next_measure = now + TimeDelta::seconds(config.measure_interval_secs.into());
        }
        self.read_sensors(now, measure_due);

        let soil_percent = self
            .soil_raw
            .value
            .map(|raw| config.soil_calibration.percent(raw));
        let climate = self.climate.value;

        let time_synced = (self.time_synced)();
        self.set_error(
            ErrorKind::TimeSync,
            !time_synced,
            "Uhrzeit nicht synchronisiert, Licht bleibt aus",
        );
        self.set_error(
            ErrorKind::NoConfig,
            !has_config,
            "Noch keine Konfiguration von ECS, automatisches Gießen ist aus",
        );

        let heater = self.decide_heater(&config, climate.map(|c| c.temperature_celsius));
        let light = self.decide_light(&config, now, time_synced);
        self.switch(Actuator::Heater, heater, false);
        self.switch(Actuator::Light, light, false);
        self.decide_pump(&config, has_config, now, soil_percent);

        let sensors = SensorValues {
            temperature_celsius: climate.map(|c| c.temperature_celsius),
            humidity_percent: climate.map(|c| c.humidity_percent),
            soil_percent,
            soil_raw: self.soil_raw.value,
        };
        let status = BoardStatus {
            version: env!("CARGO_PKG_VERSION").into(),
            time: now,
            time_synced,
            config_version: has_config.then_some(config.version),
            sensors: sensors.clone(),
            actuators: self.actuators,
            overrides: self.overrides.clone(),
            light_phase: light::active_phase(&config.light, now).map(|p| p.name.clone()),
            pump_locked: self.pump.locked,
            pumped_secs_today: self.pump.pumped_secs_today,
            errors: self.errors.values().cloned().collect(),
        };

        let mut shared = self.shared.lock().unwrap();
        shared.status = status;
        for event in self.events.drain(..) {
            shared.buffer.push(BufferEntry::Event(event));
        }
        if measure_due {
            shared.buffer.push(BufferEntry::Measurement(Measurement {
                at: now,
                sensors,
                actuators: self.actuators,
            }));
        }
    }

    fn handle(&mut self, command: Command, now: DateTime<Utc>) {
        match command {
            Command::Override(actuator, req) => {
                let slot = match actuator {
                    Actuator::Heater => &mut self.overrides.heater,
                    Actuator::Light => &mut self.overrides.light,
                    Actuator::Pump => return,
                };
                if req.minutes == 0 {
                    if slot.take().is_some() {
                        self.event(EventKind::OverrideCleared { actuator });
                    }
                } else {
                    let minutes = req.minutes.min(MAX_OVERRIDE_MINUTES);
                    *slot = Some(Override {
                        on: req.on,
                        until: now + TimeDelta::minutes(minutes.into()),
                    });
                    self.event(EventKind::OverrideSet {
                        actuator,
                        on: req.on,
                        minutes,
                    });
                }
            }
            Command::Pump { secs } => {
                let secs = secs.clamp(1, MAX_MANUAL_PUMP_SECS);
                self.start_dose(now, secs, PumpReason::Manual, None);
            }
            Command::UnlockPump => {
                if self.pump.locked {
                    self.pump.locked = false;
                    self.pump.ineffective_doses = 0;
                    self.pump.soil_before_last_dose = None;
                    self.save_pump();
                    self.event(EventKind::PumpUnlocked);
                }
                self.set_error(ErrorKind::BucketEmpty, false, "");
            }
        }
    }

    fn expire_overrides(&mut self, now: DateTime<Utc>) {
        for actuator in [Actuator::Heater, Actuator::Light] {
            let slot = match actuator {
                Actuator::Heater => &mut self.overrides.heater,
                _ => &mut self.overrides.light,
            };
            if slot.as_ref().is_some_and(|o| o.until <= now) {
                *slot = None;
                // Eigener Zeitstempel je Ereignis: InfluxDB überschreibt
                // Ereignisse gleicher Art mit gleichem Zeitstempel.
                self.event(EventKind::OverrideCleared { actuator });
            }
        }
    }

    fn read_sensors(&mut self, now: DateTime<Utc>, measure_due: bool) {
        let climate = self.climate.poll(
            now,
            measure_due,
            self.hardware.as_mut(),
            |hw| hw.reinit_climate(),
            |hw| hw.read_climate(),
        );
        if let Some(result) = climate {
            self.sensor_result(
                ErrorKind::ClimateSensor,
                "Temperatur-/Feuchtesensor",
                result,
            );
        }
        let soil = self.soil_raw.poll(
            now,
            measure_due,
            self.hardware.as_mut(),
            |hw| hw.reinit_soil(),
            |hw| hw.read_soil_raw(),
        );
        if let Some(result) = soil {
            self.sensor_result(ErrorKind::SoilSensor, "Bodenfeuchtesensor", result);
        }
    }

    fn sensor_result(&mut self, kind: ErrorKind, name: &str, result: Result<(), String>) {
        match result {
            Ok(()) => self.set_error(kind, false, ""),
            Err(err) => self.raise(kind, &format!("{name}: {err}")),
        }
    }

    /// Fail-Safe: ohne Temperaturwert bleibt die Heizung aus.
    fn decide_heater(&self, config: &BoardConfig, temperature: Option<f32>) -> bool {
        if let Some(o) = &self.overrides.heater {
            return o.on;
        }
        let Some(t) = temperature else {
            return false;
        };
        let h = &config.heater;
        if t < h.target_celsius - h.hysteresis_celsius {
            true
        } else if t > h.target_celsius + h.hysteresis_celsius {
            false
        } else {
            self.actuators.heater
        }
    }

    fn decide_light(&self, config: &BoardConfig, now: DateTime<Utc>, time_synced: bool) -> bool {
        if let Some(o) = &self.overrides.light {
            return o.on;
        }
        time_synced && light::light_on(&config.light, now)
    }

    fn decide_pump(
        &mut self,
        config: &BoardConfig,
        has_config: bool,
        now: DateTime<Utc>,
        soil: Option<f32>,
    ) {
        let today = now.date_naive();
        if self.pump.day != Some(today) {
            self.pump.day = Some(today);
            self.pump.pumped_secs_today = 0;
            self.save_pump();
        }

        // Laufenden Gießvorgang nach Ablauf beenden. Fällt der Bodensensor
        // währenddessen aus, läuft die Dosis noch zu Ende (höchstens `pump_secs`).
        if self.pump_until.is_some_and(|until| now >= until) {
            self.pump_until = None;
        }
        self.switch(Actuator::Pump, self.pump_until.is_some(), false);
        if self.pump_until.is_some() || !has_config || self.pump.locked {
            return;
        }

        let w = &config.watering;
        let Some(soil) = soil else {
            return;
        };
        if soil >= w.target_percent - w.hysteresis_percent {
            self.pump.ineffective_doses = 0;
            return;
        }
        let interval_over = self
            .pump
            .last_dose_at
            .is_none_or(|at| now - at >= TimeDelta::minutes(w.min_interval_mins.into()));
        if !interval_over || self.pump.pumped_secs_today + w.pump_secs > w.max_pump_secs_per_day {
            return;
        }

        // Noch immer zu trocken: hat der letzte Gießvorgang etwas gebracht?
        if let Some(before) = self.pump.soil_before_last_dose {
            if soil - before < w.empty_bucket.min_rise_percent {
                self.pump.ineffective_doses += 1;
            } else {
                self.pump.ineffective_doses = 0;
            }
            if self.pump.ineffective_doses >= w.empty_bucket.doses {
                self.pump.locked = true;
                self.save_pump();
                self.event(EventKind::PumpLocked);
                self.raise(
                    ErrorKind::BucketEmpty,
                    "Bodenfeuchte steigt nach dem Gießen nicht: Eimer leer? Pumpe gesperrt.",
                );
                return;
            }
        }
        self.start_dose(now, w.pump_secs, PumpReason::Auto, Some(soil));
    }

    fn start_dose(&mut self, now: DateTime<Utc>, secs: u32, reason: PumpReason, soil: Option<f32>) {
        self.pump_until = Some(now + TimeDelta::seconds(secs.into()));
        self.pump.pumped_secs_today += secs;
        self.pump.last_dose_at = Some(now);
        // Nur automatische Gießvorgänge zählen für die "Eimer leer"-Erkennung.
        self.pump.soil_before_last_dose = soil;
        self.save_pump();
        self.event(EventKind::PumpRun { secs, reason });
        self.switch(Actuator::Pump, true, false);
    }

    fn save_pump(&self) {
        if let Err(err) = self.storage.save(PUMP_STATE_FILE, &self.pump) {
            tracing::error!("Pumpenzustand konnte nicht gespeichert werden: {err:#}");
        }
    }

    /// Schaltet nur bei Änderung (oder `force`) und erzeugt ein Ereignis.
    fn switch(&mut self, actuator: Actuator, on: bool, force: bool) {
        let current = match actuator {
            Actuator::Heater => &mut self.actuators.heater,
            Actuator::Light => &mut self.actuators.light,
            Actuator::Pump => &mut self.actuators.pump,
        };
        if *current == on && !force {
            return;
        }
        match self.hardware.set(actuator, on) {
            Ok(()) => {
                let changed = *current != on;
                *current = on;
                if changed {
                    self.event(EventKind::Switched { actuator, on });
                }
                self.set_error(ErrorKind::Actuator, false, "");
            }
            Err(err) => self.raise(
                ErrorKind::Actuator,
                &format!("{actuator:?} schalten: {err:#}"),
            ),
        }
    }

    fn event(&mut self, kind: EventKind) {
        self.events.push(Event {
            at: Utc::now(),
            kind,
        });
    }

    fn raise(&mut self, kind: ErrorKind, message: &str) {
        self.set_error(kind, true, message);
    }

    /// Setzt oder löscht einen Fehler und erzeugt bei Änderung ein Ereignis.
    fn set_error(&mut self, kind: ErrorKind, active: bool, message: &str) {
        let key = error_key(kind);
        match (active, self.errors.contains_key(&key)) {
            (true, false) => {
                self.errors.insert(
                    key,
                    BoardError {
                        kind,
                        message: message.into(),
                        since: Utc::now(),
                    },
                );
                self.event(EventKind::ErrorRaised {
                    kind,
                    message: message.into(),
                });
            }
            (false, true) => {
                self.errors.remove(&key);
                self.event(EventKind::ErrorCleared { kind });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Buffer;
    use autoweed_shared::OverrideRequest;
    use chrono::TimeZone;

    /// Sensorwerte, die der Test von außen setzt; `None` = Sensor fällt aus.
    #[derive(Default)]
    struct Fake {
        climate: Option<Climate>,
        soil_raw: Option<i16>,
        states: ActuatorStates,
    }

    struct FakeHardware(Arc<Mutex<Fake>>);

    impl Hardware for FakeHardware {
        fn read_climate(&mut self) -> anyhow::Result<Climate> {
            self.0
                .lock()
                .unwrap()
                .climate
                .ok_or_else(|| anyhow::anyhow!("weg"))
        }
        fn read_soil_raw(&mut self) -> anyhow::Result<i16> {
            self.0
                .lock()
                .unwrap()
                .soil_raw
                .ok_or_else(|| anyhow::anyhow!("weg"))
        }
        fn reinit_climate(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        fn reinit_soil(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        fn set(&mut self, actuator: Actuator, on: bool) -> anyhow::Result<()> {
            let states = &mut self.0.lock().unwrap().states;
            match actuator {
                Actuator::Heater => states.heater = on,
                Actuator::Light => states.light = on,
                Actuator::Pump => states.pump = on,
            }
            Ok(())
        }
    }

    struct Setup {
        controller: Controller,
        fake: Arc<Mutex<Fake>>,
        shared: Arc<Mutex<Shared>>,
        now: DateTime<Utc>,
    }

    impl Setup {
        fn new(config: BoardConfig) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "autoweed-test-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            let storage = Storage::new(dir).unwrap();
            let fake = Arc::new(Mutex::new(Fake {
                climate: Some(climate(24.0)),
                soil_raw: Some(raw_for(&config, 50.0)),
                ..Default::default()
            }));
            let shared = Arc::new(Mutex::new(Shared::new(config, true, Buffer::new(1))));
            let controller = Controller::new(
                Box::new(FakeHardware(fake.clone())),
                shared.clone(),
                storage,
                Box::new(|| true),
            );
            Self {
                controller,
                fake,
                shared,
                now: Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap(),
            }
        }

        /// Lässt die Zeit sekundenweise laufen, wie die echte Schleife.
        fn run_secs(&mut self, secs: i64) {
            for _ in 0..secs {
                self.controller.tick(self.now);
                self.now += TimeDelta::seconds(1);
            }
        }

        fn states(&self) -> ActuatorStates {
            self.fake.lock().unwrap().states
        }

        fn set_temperature(&self, t: f32) {
            self.fake.lock().unwrap().climate = Some(climate(t));
        }

        fn set_soil(&self, percent: f32) {
            let raw = raw_for(&self.shared.lock().unwrap().config, percent);
            self.fake.lock().unwrap().soil_raw = Some(raw);
        }
    }

    fn climate(t: f32) -> Climate {
        Climate {
            temperature_celsius: t,
            humidity_percent: 50.0,
        }
    }

    fn raw_for(config: &BoardConfig, percent: f32) -> i16 {
        let c = &config.soil_calibration;
        let (dry, wet) = (f32::from(c.dry_raw), f32::from(c.wet_raw));
        (dry - percent / 100.0 * (dry - wet)).round() as i16
    }

    #[test]
    fn heater_hysteresis() {
        // Ziel 24 °C, Hysterese 1 °C
        let mut s = Setup::new(BoardConfig::default());
        s.set_temperature(22.9);
        s.run_secs(6);
        assert!(s.states().heater);
        s.set_temperature(24.5);
        s.run_secs(6);
        assert!(s.states().heater, "innerhalb der Hysterese bleibt sie an");
        s.set_temperature(25.1);
        s.run_secs(6);
        assert!(!s.states().heater);
        s.set_temperature(23.5);
        s.run_secs(6);
        assert!(!s.states().heater, "innerhalb der Hysterese bleibt sie aus");
    }

    #[test]
    fn heater_off_without_temperature() {
        let mut s = Setup::new(BoardConfig::default());
        s.set_temperature(10.0);
        s.run_secs(6);
        assert!(s.states().heater);
        s.fake.lock().unwrap().climate = None;
        s.run_secs(6);
        assert!(!s.states().heater);
        let errors = &s.shared.lock().unwrap().status.errors;
        assert!(errors.iter().any(|e| e.kind == ErrorKind::ClimateSensor));
    }

    #[test]
    fn pump_doses_and_respects_interval() {
        // Ziel 50 %, Hysterese 10 %, 10 s, alle 60 min, 120 s pro Tag
        let mut s = Setup::new(BoardConfig::default());
        s.set_soil(35.0);
        s.run_secs(1);
        assert!(s.states().pump);
        s.run_secs(10);
        assert!(!s.states().pump, "nach pump_secs aus");
        s.run_secs(30 * 60);
        assert!(!s.states().pump, "Mindestabstand noch nicht vorbei");
        // Feuchte ist gestiegen, aber noch zu trocken → zweiter Gießvorgang.
        s.set_soil(38.0);
        s.run_secs(31 * 60);
        assert_eq!(s.controller.pump.pumped_secs_today, 20);
        assert_eq!(s.controller.pump.ineffective_doses, 0);
    }

    #[test]
    fn empty_bucket_locks_pump() {
        // 3 Gießvorgänge in Folge ohne Anstieg um 2 % → Sperre
        let mut s = Setup::new(BoardConfig::default());
        s.set_soil(30.0);
        s.run_secs(4 * 61 * 60);
        assert!(s.controller.pump.locked);
        // Erster Gießvorgang + 3 wirkungslose: der dritte wird nicht mehr ausgeführt.
        assert_eq!(s.controller.pump.pumped_secs_today, 30);
        assert!(!s.states().pump);
        assert!(s.shared.lock().unwrap().status.pump_locked);

        s.shared.lock().unwrap().commands.push(Command::UnlockPump);
        s.run_secs(1);
        assert!(!s.controller.pump.locked);
        let errors = &s.shared.lock().unwrap().status.errors;
        assert!(!errors.iter().any(|e| e.kind == ErrorKind::BucketEmpty));
    }

    #[test]
    fn daily_limit() {
        let mut config = BoardConfig::default();
        config.watering.min_interval_mins = 1;
        config.watering.max_pump_secs_per_day = 25;
        config.watering.empty_bucket.doses = 100;
        let mut s = Setup::new(config);
        s.set_soil(30.0);
        s.run_secs(30 * 60);
        assert_eq!(s.controller.pump.pumped_secs_today, 20);
    }

    #[test]
    fn no_watering_without_soil_value() {
        let mut s = Setup::new(BoardConfig::default());
        s.fake.lock().unwrap().soil_raw = None;
        s.run_secs(120);
        assert_eq!(s.controller.pump.pumped_secs_today, 0);
        assert!(!s.states().pump);
    }

    #[test]
    fn overrides_expire_with_distinct_events() {
        let mut s = Setup::new(BoardConfig::default());
        for actuator in [Actuator::Heater, Actuator::Light] {
            s.shared.lock().unwrap().commands.push(Command::Override(
                actuator,
                OverrideRequest {
                    on: true,
                    minutes: 1,
                },
            ));
        }
        s.set_temperature(30.0);
        s.run_secs(2);
        assert!(s.states().heater, "Übersteuerung schlägt die Regelung");
        s.run_secs(60);
        assert!(!s.states().heater);
        let entries = s.shared.lock().unwrap().buffer.take(0, 1).entries;
        let cleared: Vec<_> = entries
            .iter()
            .filter_map(|e| match e {
                BufferEntry::Event(e) if matches!(e.kind, EventKind::OverrideCleared { .. }) => {
                    Some(e.at)
                }
                _ => None,
            })
            .collect();
        assert_eq!(cleared.len(), 2);
        assert_ne!(cleared[0], cleared[1]);
    }

    #[test]
    fn override_duration_is_capped() {
        let mut s = Setup::new(BoardConfig::default());
        s.shared.lock().unwrap().commands.push(Command::Override(
            Actuator::Light,
            OverrideRequest {
                on: true,
                minutes: u32::MAX,
            },
        ));
        s.run_secs(1);
        let until = s.controller.overrides.light.as_ref().unwrap().until;
        assert!(until - s.now <= TimeDelta::minutes(MAX_OVERRIDE_MINUTES.into()));
    }
}
