//! Konfiguration, die der Server verwaltet und an das Board schickt (`PUT /api/v1/config`).
//!
//! Das Board regelt ausschließlich mit dieser Konfiguration und der aktuellen
//! Uhrzeit. Es hält selbst keinen Zustand, der vom Server abweichen könnte,
//! z. B. wird die aktive Lichtphase aus den Angaben hier berechnet.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BoardConfig {
    /// Wird vom Server bei jeder Änderung erhöht. Das Board meldet die aktive
    /// Version im Status, so erkennt der Server, ob die Konfiguration angekommen ist.
    pub version: u32,
    /// Abstand zwischen zwei Messungen, 2–10 s.
    pub measure_interval_secs: u32,
    pub heater: HeaterConfig,
    pub watering: WateringConfig,
    pub soil_calibration: SoilCalibration,
    pub light: LightSchedule,
}

/// Zweipunktregelung: an unter `target − hysteresis`, aus über `target + hysteresis`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HeaterConfig {
    pub target_celsius: f32,
    pub hysteresis_celsius: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct WateringConfig {
    /// Ziel-Bodenfeuchte in Prozent (nach Kalibrierung, 0 = trocken, 100 = nass).
    pub target_percent: f32,
    /// Gegossen wird unter `target − hysteresis`.
    pub hysteresis_percent: f32,
    /// Laufzeit der Pumpe pro Gießvorgang.
    pub pump_secs: u32,
    /// Mindestabstand zwischen zwei Gießvorgängen, damit das Wasser einziehen kann.
    pub min_interval_mins: u32,
    /// Obergrenze der Pumpenlaufzeit pro Tag (UTC), schützt vor Überschwemmung.
    pub max_pump_secs_per_day: u32,
    pub empty_bucket: EmptyBucketDetection,
}

/// Steigt die Feuchte nach `doses` Gießvorgängen in Folge jeweils um weniger
/// als `min_rise_percent`, gilt der Eimer als leer und die Pumpe wird gesperrt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EmptyBucketDetection {
    pub doses: u32,
    pub min_rise_percent: f32,
}

/// Rohwerte des ADS1115 für trockene und nasse Erde. Kapazitive Sensoren
/// liefern bei trockener Erde den höheren Wert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SoilCalibration {
    pub dry_raw: i16,
    pub wet_raw: i16,
}

/// Lichtzyklus. Die Lampe ist ab `start_minute_utc` für `on_minutes` an und
/// den Rest des Tages aus. Gerechnet wird in UTC, damit jeder Tag genau 24 h
/// hat; bei der Zeitumstellung verschiebt sich die Anzeige in Ortszeit um 1 h.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(export)]
pub enum LightSchedule {
    /// Ein fester Zyklus für den ganzen Grow.
    Autoflower {
        start_minute_utc: u16,
        on_minutes: u16,
    },
    /// Phasen nacheinander, z. B. Wachstum 18/6, dann Blüte 12/12.
    Photoperiod {
        start_minute_utc: u16,
        phases: Vec<LightPhase>,
        /// Index in `phases`, den der Benutzer zuletzt gesetzt hat.
        active_phase: u32,
        /// Seit wann `active_phase` läuft. Phasen mit `duration_days` gehen
        /// danach automatisch in die nächste über.
        phase_started_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LightPhase {
    pub name: String,
    pub on_minutes: u16,
    /// `None`: Wechsel nur manuell. `Some(n)`: nach n Tagen automatisch weiter.
    pub duration_days: Option<u32>,
}

const MINUTES_PER_DAY: u16 = 24 * 60;

impl BoardConfig {
    /// Prüft die Konfiguration, bevor der Server sie speichert bzw. das Board sie übernimmt.
    /// Gibt alle Fehler auf einmal zurück, damit die WebUI sie gesammelt anzeigen kann.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut check = |ok: bool, msg: &str| {
            if !ok {
                errors.push(msg.to_owned());
            }
        };

        check(
            (2..=10).contains(&self.measure_interval_secs),
            "Messintervall muss zwischen 2 und 10 s liegen",
        );

        let h = &self.heater;
        check(
            (0.0..=40.0).contains(&h.target_celsius),
            "Zieltemperatur muss zwischen 0 und 40 °C liegen",
        );
        check(
            h.hysteresis_celsius > 0.0 && h.hysteresis_celsius <= 5.0,
            "Temperatur-Hysterese muss zwischen 0 und 5 °C liegen",
        );

        let w = &self.watering;
        check(
            (0.0..=100.0).contains(&w.target_percent),
            "Ziel-Bodenfeuchte muss zwischen 0 und 100 % liegen",
        );
        check(
            w.hysteresis_percent > 0.0 && w.hysteresis_percent < w.target_percent,
            "Feuchte-Hysterese muss größer 0 und kleiner als die Ziel-Bodenfeuchte sein",
        );
        check(
            (1..=120).contains(&w.pump_secs),
            "Pumpenlaufzeit muss zwischen 1 und 120 s liegen",
        );
        check(
            w.min_interval_mins >= 1,
            "Mindestabstand zwischen Gießvorgängen muss mindestens 1 min sein",
        );
        check(
            w.max_pump_secs_per_day >= w.pump_secs,
            "Tageslimit der Pumpe muss mindestens eine Pumpenlaufzeit betragen",
        );
        check(
            w.empty_bucket.doses >= 1,
            "„Eimer leer“-Erkennung braucht mindestens 1 Gießvorgang",
        );
        check(
            w.empty_bucket.min_rise_percent >= 0.0,
            "Mindestanstieg der Feuchte darf nicht negativ sein",
        );

        check(
            self.soil_calibration.dry_raw != self.soil_calibration.wet_raw,
            "Kalibrierwerte für trocken und nass dürfen nicht gleich sein",
        );

        match &self.light {
            LightSchedule::Autoflower {
                start_minute_utc,
                on_minutes,
            } => {
                check(
                    *start_minute_utc < MINUTES_PER_DAY,
                    "Startzeit des Lichts muss vor 24:00 liegen",
                );
                check(
                    *on_minutes <= MINUTES_PER_DAY,
                    "Lichtdauer darf höchstens 24 h sein",
                );
            }
            LightSchedule::Photoperiod {
                start_minute_utc,
                phases,
                active_phase,
                ..
            } => {
                check(
                    *start_minute_utc < MINUTES_PER_DAY,
                    "Startzeit des Lichts muss vor 24:00 liegen",
                );
                check(
                    !phases.is_empty(),
                    "Es muss mindestens eine Lichtphase geben",
                );
                check(
                    (*active_phase as usize) < phases.len(),
                    "Aktive Lichtphase existiert nicht",
                );
                for phase in phases {
                    check(
                        !phase.name.trim().is_empty(),
                        "Jede Lichtphase braucht einen Namen",
                    );
                    check(
                        phase.on_minutes <= MINUTES_PER_DAY,
                        "Lichtdauer darf höchstens 24 h sein",
                    );
                    check(
                        phase.duration_days != Some(0),
                        "Phasendauer muss mindestens 1 Tag sein",
                    );
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for BoardConfig {
    /// Startwerte, bis der Benutzer in der WebUI etwas anderes einstellt.
    fn default() -> Self {
        Self {
            version: 0,
            measure_interval_secs: 5,
            heater: HeaterConfig {
                target_celsius: 24.0,
                hysteresis_celsius: 1.0,
            },
            watering: WateringConfig {
                target_percent: 50.0,
                hysteresis_percent: 10.0,
                pump_secs: 10,
                min_interval_mins: 60,
                max_pump_secs_per_day: 120,
                empty_bucket: EmptyBucketDetection {
                    doses: 3,
                    min_rise_percent: 2.0,
                },
            },
            // Typische Werte eines kapazitiven v1.2-Sensors an 3,3 V; werden
            // bei der Inbetriebnahme (8.1) kalibriert.
            soil_calibration: SoilCalibration {
                dry_raw: 20_000,
                wet_raw: 9_000,
            },
            // 18/6, Licht an um 04:00 UTC (06:00 MEZ).
            light: LightSchedule::Autoflower {
                start_minute_utc: 4 * 60,
                on_minutes: 18 * 60,
            },
        }
    }
}

impl SoilCalibration {
    /// Rechnet einen ADS1115-Rohwert in Bodenfeuchte (0–100 %) um.
    pub fn percent(&self, raw: i16) -> f32 {
        let (dry, wet) = (f32::from(self.dry_raw), f32::from(self.wet_raw));
        ((dry - f32::from(raw)) / (dry - wet) * 100.0).clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid() {
        assert_eq!(BoardConfig::default().validate(), Ok(()));
    }

    #[test]
    fn collects_all_errors() {
        let mut config = BoardConfig {
            measure_interval_secs: 1,
            ..Default::default()
        };
        config.watering.pump_secs = 0;
        config.soil_calibration.wet_raw = config.soil_calibration.dry_raw;
        assert_eq!(config.validate().unwrap_err().len(), 3);
    }

    #[test]
    fn soil_percent_is_clamped() {
        let c = SoilCalibration {
            dry_raw: 20_000,
            wet_raw: 10_000,
        };
        assert_eq!(c.percent(20_000), 0.0);
        assert_eq!(c.percent(15_000), 50.0);
        assert_eq!(c.percent(10_000), 100.0);
        assert_eq!(c.percent(25_000), 0.0);
        assert_eq!(c.percent(5_000), 100.0);
    }
}
