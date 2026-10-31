//! Simuliertes Grow-Zelt für die Entwicklung ohne Pi.
//!
//! Sehr grobes Modell: Die Temperatur fällt in Richtung Garagentemperatur und
//! steigt mit Heizung und Lampe, die Erde trocknet aus und wird durch die
//! Pumpe feucht. `speed` beschleunigt die Zeit, `fail_rate` lässt Messungen
//! zufällig fehlschlagen (zum Testen des Back-offs).

use std::time::Instant;

use anyhow::{Result, bail};
use autoweed_shared::{Actuator, BoardConfig};

use super::{Climate, Hardware};

const AMBIENT_CELSIUS: f32 = 12.0;

pub struct SimHardware {
    speed: f32,
    fail_rate: f32,
    rng: u64,
    last_step: Instant,
    temperature: f32,
    humidity: f32,
    soil_percent: f32,
    heater: bool,
    light: bool,
    pump: bool,
}

impl SimHardware {
    pub fn new(speed: f32, fail_rate: f32) -> Self {
        Self {
            speed,
            fail_rate,
            rng: 0x9E37_79B9_7F4A_7C15,
            last_step: Instant::now(),
            temperature: 18.0,
            humidity: 55.0,
            soil_percent: 45.0,
            heater: false,
            light: false,
            pump: false,
        }
    }

    /// Xorshift, reicht für Rauschen und Fehlersimulation.
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    fn noise(&mut self, amplitude: f32) -> f32 {
        (self.random() - 0.5) * 2.0 * amplitude
    }

    fn step(&mut self) {
        let dt = self.last_step.elapsed().as_secs_f32() * self.speed;
        self.last_step = Instant::now();

        let heating = if self.heater { 0.01 } else { 0.0 } + if self.light { 0.002 } else { 0.0 };
        self.temperature += dt * ((AMBIENT_CELSIUS - self.temperature) * 0.0005 + heating);

        let target_humidity = if self.heater { 45.0 } else { 60.0 };
        self.humidity += dt * (target_humidity - self.humidity) * 0.001;

        let drying = if self.light { 0.0008 } else { 0.0004 };
        let watering = if self.pump { 0.8 } else { 0.0 };
        self.soil_percent = (self.soil_percent + dt * (watering - drying)).clamp(0.0, 100.0);
    }

    fn maybe_fail(&mut self, what: &str) -> Result<()> {
        if self.random() < self.fail_rate {
            bail!("simulierter Fehler beim Lesen von {what}");
        }
        Ok(())
    }
}

impl Hardware for SimHardware {
    fn read_climate(&mut self) -> Result<Climate> {
        self.step();
        self.maybe_fail("Temperatur/Luftfeuchte")?;
        Ok(Climate {
            temperature_celsius: self.temperature + self.noise(0.05),
            humidity_percent: (self.humidity + self.noise(0.5)).clamp(0.0, 100.0),
        })
    }

    fn read_soil_raw(&mut self) -> Result<i16> {
        self.step();
        self.maybe_fail("Bodenfeuchte")?;
        // Rückrechnung mit der Standard-Kalibrierung.
        let cal = BoardConfig::default().soil_calibration;
        let (dry, wet) = (f32::from(cal.dry_raw), f32::from(cal.wet_raw));
        let percent = self.soil_percent + self.noise(0.3);
        Ok((dry - (dry - wet) * percent / 100.0) as i16)
    }

    fn reinit_climate(&mut self) -> Result<()> {
        Ok(())
    }

    fn reinit_soil(&mut self) -> Result<()> {
        Ok(())
    }

    fn set(&mut self, actuator: Actuator, on: bool) -> Result<()> {
        self.step();
        match actuator {
            Actuator::Heater => self.heater = on,
            Actuator::Light => self.light = on,
            Actuator::Pump => self.pump = on,
        }
        Ok(())
    }
}
