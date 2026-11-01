//! Request- und Response-Typen der Board-API und der ECS-API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Aktuelle Messwerte. `None`, wenn der Sensor gerade ausgefallen ist.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SensorValues {
    pub temperature_celsius: Option<f32>,
    pub humidity_percent: Option<f32>,
    pub soil_percent: Option<f32>,
    /// Rohwert des ADS1115, für die Kalibrierung in der WebUI.
    pub soil_raw: Option<i16>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActuatorStates {
    pub heater: bool,
    pub light: bool,
    pub pump: bool,
}

/// Manuelle Übersteuerung von Heizung oder Licht, aktiv bis `until`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Override {
    pub on: bool,
    pub until: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Overrides {
    pub heater: Option<Override>,
    pub light: Option<Override>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ErrorKind {
    /// Temperatur-/Luftfeuchtesensor liefert keine Werte.
    ClimateSensor,
    /// Bodenfeuchtesensor bzw. ADC liefert keine Werte.
    SoilSensor,
    /// Uhrzeit nicht synchronisiert, das Licht bleibt aus.
    TimeSync,
    /// Feuchte steigt nach dem Gießen nicht, die Pumpe ist gesperrt.
    BucketEmpty,
    /// Noch keine Konfiguration von ECS, automatisches Gießen ist aus.
    NoConfig,
    /// Ein Aktor ließ sich nicht schalten.
    Actuator,
}

/// Ein aktiver Fehler auf dem Board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BoardError {
    pub kind: ErrorKind,
    pub message: String,
    pub since: DateTime<Utc>,
}

/// Antwort von `GET /api/v1/status` (Board).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BoardStatus {
    /// Software-Version des Boards.
    pub version: String,
    pub time: DateTime<Utc>,
    pub time_synced: bool,
    /// Version der aktiven Konfiguration, `None` = noch keine von ECS erhalten.
    pub config_version: Option<u32>,
    pub sensors: SensorValues,
    pub actuators: ActuatorStates,
    pub overrides: Overrides,
    /// Name der aktiven Lichtphase (nur im Modus Photoperiod).
    pub light_phase: Option<String>,
    pub pump_locked: bool,
    /// Pumpenlaufzeit heute (UTC-Tag) in Sekunden.
    pub pumped_secs_today: u32,
    pub errors: Vec<BoardError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Actuator {
    Heater,
    Light,
    Pump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PumpReason {
    Auto,
    Manual,
}

/// Was auf dem Board passiert ist, für die Ereignisliste in der WebUI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum EventKind {
    Started {
        version: String,
    },
    Switched {
        actuator: Actuator,
        on: bool,
    },
    PumpRun {
        secs: u32,
        reason: PumpReason,
    },
    PumpLocked,
    PumpUnlocked,
    ErrorRaised {
        kind: ErrorKind,
        message: String,
    },
    ErrorCleared {
        kind: ErrorKind,
    },
    ConfigApplied {
        version: u32,
    },
    OverrideSet {
        actuator: Actuator,
        on: bool,
        minutes: u32,
    },
    OverrideCleared {
        actuator: Actuator,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Event {
    pub at: DateTime<Utc>,
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Ein Messpunkt im Puffer des Boards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Measurement {
    pub at: DateTime<Utc>,
    pub sensors: SensorValues,
    pub actuators: ActuatorStates,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "entry", rename_all = "snake_case")]
pub enum BufferEntry {
    Measurement(Measurement),
    Event(Event),
}

/// Query von `GET /api/v1/measurements` (Board).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeasurementsQuery {
    /// Alles bis einschließlich `since` gilt als abgeholt und wird verworfen.
    #[serde(default)]
    pub since: u64,
    /// Passt `boot_id` nicht zum laufenden Board (Neustart), wird `since` ignoriert.
    #[serde(default)]
    pub boot_id: u64,
}

/// Antwort von `GET /api/v1/measurements` (Board).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementBatch {
    pub boot_id: u64,
    /// Beim nächsten Abruf als `since` mitschicken.
    pub next_cursor: u64,
    /// Es liegen noch weitere Einträge vor, sofort erneut abrufen.
    pub more: bool,
    pub entries: Vec<BufferEntry>,
}

/// `POST /api/v1/override/{heater,light}`. `minutes == 0` beendet die Übersteuerung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OverrideRequest {
    pub on: bool,
    pub minutes: u32,
}

/// `POST /api/v1/override/pump`: einmal gießen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PumpRequest {
    pub secs: u32,
}

/// Höchste Laufzeit eines manuellen Gießvorgangs.
pub const MAX_MANUAL_PUMP_SECS: u32 = 120;

/// Längste manuelle Übersteuerung von Heizung oder Licht (24 h). Längere
/// Anfragen kürzt das Board, ECS lehnt sie ab.
pub const MAX_OVERRIDE_MINUTES: u32 = 24 * 60;

// ---------------------------------------------------------------------------
// ECS-API (für das Frontend)
// ---------------------------------------------------------------------------

/// `GET /api/status` (ECS).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EcsStatus {
    pub board_reachable: bool,
    pub last_contact: Option<DateTime<Utc>>,
    /// Letzter bekannter Status des Boards (auch wenn es gerade nicht erreichbar ist).
    pub board: Option<BoardStatus>,
    /// Version der auf ECS gespeicherten Konfiguration.
    pub config_version: u32,
    /// Das Board verwendet die auf ECS gespeicherte Version.
    pub config_synced: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionInfo {
    pub authenticated: bool,
}

/// Fehlerantwort der ECS-API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub message: String,
    /// Einzelne Validierungsfehler (z. B. beim Speichern der Konfiguration).
    pub details: Vec<String>,
}

/// Query von `GET /api/history` und `GET /api/events`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RangeQuery {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Ein aggregierter Punkt im Verlauf. Aktoren als Anteil der Zeit, in der sie an waren (0–1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HistoryPoint {
    pub at: DateTime<Utc>,
    pub temperature_celsius: Option<f32>,
    pub humidity_percent: Option<f32>,
    pub soil_percent: Option<f32>,
    pub heater: Option<f32>,
    pub light: Option<f32>,
    pub pump: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct History {
    /// Breite eines Aggregationsfensters in Sekunden.
    pub window_secs: u32,
    pub points: Vec<HistoryPoint>,
}
