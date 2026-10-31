//! Zugriff auf Sensoren und Aktoren.
//!
//! Die Regelung kennt nur den Trait [`Hardware`]. Auf dem Pi steckt
//! [`pi::PiHardware`] dahinter, auf dem Entwicklungsrechner [`sim::SimHardware`].

pub mod pi;
pub mod sim;

use anyhow::Result;
use autoweed_shared::Actuator;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Climate {
    pub temperature_celsius: f32,
    pub humidity_percent: f32,
}

pub trait Hardware: Send {
    fn read_climate(&mut self) -> Result<Climate>;
    /// Rohwert des ADS1115 (Kanal 0) für den Bodenfeuchtesensor.
    fn read_soil_raw(&mut self) -> Result<i16>;
    /// Sensor nach einem Fehler neu initialisieren.
    fn reinit_climate(&mut self) -> Result<()>;
    fn reinit_soil(&mut self) -> Result<()>;
    fn set(&mut self, actuator: Actuator, on: bool) -> Result<()>;
}
