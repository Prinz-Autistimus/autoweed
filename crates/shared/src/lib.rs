//! Typen, die Board, ECS-Backend und Frontend gemeinsam verwenden.
//!
//! Alle Typen mit `#[ts(export)]` werden bei `cargo test -p autoweed-shared`
//! als TypeScript nach `frontend/src/generated/` geschrieben.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Antwort von `GET /api/v1/health` (Board und ECS).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Health {
    /// `"board"` oder `"ecs"`.
    pub service: String,
    /// Version aus `Cargo.toml`.
    pub version: String,
}
