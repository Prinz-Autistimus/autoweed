//! Echte Hardware am Raspberry Pi: SHT31/SHT40 und ADS1115 über I²C,
//! Relais (Heizung, Lampe) und MOSFET (Pumpe) über GPIO.

use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use autoweed_shared::Actuator;
use rppal::gpio::{Gpio, Level, OutputPin};
use rppal::i2c::I2c;

use super::{Climate, Hardware};

const SHT_ADDR: u16 = 0x44;
const ADS1115_ADDR: u16 = 0x48;
/// Plausibler Bereich des Bodensensors (±4,096 V: 1 LSB = 125 µV). Ein
/// abgezogener Sensor liest über den 1-MΩ-Pull-down an A0 fast 0, ein
/// Kurzschluss nach 3,3 V ca. 26 400. Ohne diese Prüfung würde die Pumpe
/// mit einem Phantomwert gießen.
const SOIL_RAW_MIN: i16 = 2_000;
const SOIL_RAW_MAX: i16 = 25_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClimateSensor {
    Sht31,
    Sht40,
}

/// BCM-Nummern der Ausgänge.
#[derive(Debug, Clone, Copy)]
pub struct Pins {
    pub heater: u8,
    pub light: u8,
    pub pump: u8,
}

pub struct PiHardware {
    i2c: I2c,
    climate_sensor: ClimateSensor,
    heater: OutputPin,
    light: OutputPin,
    pump: OutputPin,
    /// Relais-Module mit Optokoppler schalten meist bei LOW.
    relay_active_low: bool,
}

impl PiHardware {
    pub fn new(climate_sensor: ClimateSensor, pins: Pins, relay_active_low: bool) -> Result<Self> {
        let gpio = Gpio::new().context("GPIO nicht verfügbar (/dev/gpiomem)")?;
        let relay_off = if relay_active_low {
            Level::High
        } else {
            Level::Low
        };
        // Alle Ausgänge starten im Zustand "aus" und behalten ihren Zustand,
        // wenn das Programm endet (kein Zurücksetzen auf Eingang). Der Pegel
        // wird vor dem Umschalten auf Ausgang gesetzt, sonst zieht ein Relais
        // beim Start kurz an.
        let output = |pin: u8, off: Level| -> Result<OutputPin> {
            let pin = gpio.get(pin).with_context(|| format!("GPIO {pin}"))?;
            let mut pin = match off {
                Level::High => pin.into_output_high(),
                Level::Low => pin.into_output_low(),
            };
            pin.set_reset_on_drop(false);
            Ok(pin)
        };
        Ok(Self {
            i2c: I2c::new().context("I²C nicht verfügbar (/dev/i2c-1)")?,
            climate_sensor,
            heater: output(pins.heater, relay_off)?,
            light: output(pins.light, relay_off)?,
            pump: output(pins.pump, Level::Low)?,
            relay_active_low,
        })
    }

    /// Schreibt `cmd`, wartet und liest dann `buf` von `addr`.
    fn transfer(&mut self, addr: u16, cmd: &[u8], wait: Duration, buf: &mut [u8]) -> Result<()> {
        self.i2c.set_slave_address(addr)?;
        self.i2c.write(cmd)?;
        sleep(wait);
        self.i2c.read(buf)?;
        Ok(())
    }
}

/// CRC-8 der Sensirion-Sensoren (Polynom 0x31, Start 0xFF).
fn sensirion_crc(data: &[u8]) -> u8 {
    let mut crc = 0xFFu8;
    for byte in data {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x31
            } else {
                crc << 1
            };
        }
    }
    crc
}

impl Hardware for PiHardware {
    fn read_climate(&mut self) -> Result<Climate> {
        let mut buf = [0u8; 6];
        let (cmd, wait): (&[u8], _) = match self.climate_sensor {
            // Single Shot, hohe Wiederholgenauigkeit, ohne Clock Stretching
            ClimateSensor::Sht31 => (&[0x24, 0x00], Duration::from_millis(16)),
            ClimateSensor::Sht40 => (&[0xFD], Duration::from_millis(10)),
        };
        self.transfer(SHT_ADDR, cmd, wait, &mut buf)?;
        if sensirion_crc(&buf[0..2]) != buf[2] || sensirion_crc(&buf[3..5]) != buf[5] {
            bail!("CRC-Fehler beim Temperatur-/Feuchtesensor");
        }
        let raw_t = f32::from(u16::from_be_bytes([buf[0], buf[1]]));
        let raw_h = f32::from(u16::from_be_bytes([buf[3], buf[4]]));
        let temperature_celsius = -45.0 + 175.0 * raw_t / 65535.0;
        let humidity_percent = match self.climate_sensor {
            ClimateSensor::Sht31 => 100.0 * raw_h / 65535.0,
            ClimateSensor::Sht40 => -6.0 + 125.0 * raw_h / 65535.0,
        }
        .clamp(0.0, 100.0);
        Ok(Climate {
            temperature_celsius,
            humidity_percent,
        })
    }

    fn read_soil_raw(&mut self) -> Result<i16> {
        // Config-Register: Single Shot, AIN0 gegen GND, ±4,096 V, 128 SPS, Comparator aus
        self.i2c.set_slave_address(ADS1115_ADDR)?;
        self.i2c.write(&[0x01, 0xC3, 0x83])?;
        let mut buf = [0u8; 2];
        self.transfer(ADS1115_ADDR, &[0x00], Duration::from_millis(9), &mut buf)?;
        let raw = i16::from_be_bytes(buf);
        if !(SOIL_RAW_MIN..=SOIL_RAW_MAX).contains(&raw) {
            bail!("Rohwert {raw} unplausibel, Sensor abgezogen oder Kurzschluss?");
        }
        Ok(raw)
    }

    fn reinit_climate(&mut self) -> Result<()> {
        self.i2c = I2c::new()?;
        self.i2c.set_slave_address(SHT_ADDR)?;
        // Soft Reset
        match self.climate_sensor {
            ClimateSensor::Sht31 => self.i2c.write(&[0x30, 0xA2])?,
            ClimateSensor::Sht40 => self.i2c.write(&[0x94])?,
        };
        sleep(Duration::from_millis(2));
        Ok(())
    }

    fn reinit_soil(&mut self) -> Result<()> {
        self.i2c = I2c::new()?;
        Ok(())
    }

    fn set(&mut self, actuator: Actuator, on: bool) -> Result<()> {
        let relay_level = if on != self.relay_active_low {
            Level::High
        } else {
            Level::Low
        };
        match actuator {
            Actuator::Heater => self.heater.write(relay_level),
            Actuator::Light => self.light.write(relay_level),
            Actuator::Pump => self.pump.write(if on { Level::High } else { Level::Low }),
        }
        Ok(())
    }
}
