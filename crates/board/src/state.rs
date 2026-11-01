//! Zustand, den sich Regelschleife und HTTP-API teilen, sowie Persistenz.

use std::collections::VecDeque;
use std::path::PathBuf;

use anyhow::Result;
use autoweed_shared::{
    Actuator, BoardConfig, BoardStatus, BufferEntry, MeasurementBatch, OverrideRequest,
};
use serde::{Serialize, de::DeserializeOwned};

/// Ca. 48 h Messwerte bei 5 s Intervall plus Ereignisse.
const BUFFER_CAPACITY: usize = 40_000;
/// Höchstens so viele Einträge pro Abruf.
const BATCH_LIMIT: usize = 2_000;

/// Befehle der API an die Regelschleife.
#[derive(Debug, Clone)]
pub enum Command {
    Override(Actuator, OverrideRequest),
    Pump { secs: u32 },
    UnlockPump,
}

pub struct Shared {
    pub config: BoardConfig,
    /// `false`, solange nur die Standardwerte gelten.
    pub config_from_ecs: bool,
    pub status: BoardStatus,
    pub buffer: Buffer,
    pub commands: Vec<Command>,
}

impl Shared {
    pub fn new(config: BoardConfig, config_from_ecs: bool, buffer: Buffer) -> Self {
        Self {
            config,
            config_from_ecs,
            status: BoardStatus {
                version: env!("CARGO_PKG_VERSION").into(),
                time: chrono::Utc::now(),
                time_synced: false,
                config_version: None,
                sensors: Default::default(),
                actuators: Default::default(),
                overrides: Default::default(),
                light_phase: None,
                pump_locked: false,
                pumped_secs_today: 0,
                errors: Vec::new(),
            },
            buffer,
            commands: Vec::new(),
        }
    }
}

/// Messwerte und Ereignisse, bis ECS sie abholt.
pub struct Buffer {
    boot_id: u64,
    next_seq: u64,
    entries: VecDeque<(u64, BufferEntry)>,
}

impl Buffer {
    pub fn new(boot_id: u64) -> Self {
        Self {
            boot_id,
            next_seq: 1,
            entries: VecDeque::new(),
        }
    }

    pub fn push(&mut self, entry: BufferEntry) {
        if self.entries.len() >= BUFFER_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back((self.next_seq, entry));
        self.next_seq += 1;
    }

    /// Verwirft alles bis `since` (von ECS bestätigt) und liefert den Rest.
    pub fn take(&mut self, since: u64, boot_id: u64) -> MeasurementBatch {
        let since = if boot_id == self.boot_id { since } else { 0 };
        while self.entries.front().is_some_and(|(seq, _)| *seq <= since) {
            self.entries.pop_front();
        }
        let entries: Vec<_> = self
            .entries
            .iter()
            .take(BATCH_LIMIT)
            .map(|(_, e)| e.clone())
            .collect();
        let next_cursor = self
            .entries
            .get(entries.len().saturating_sub(1))
            .filter(|_| !entries.is_empty())
            .map_or(since, |(seq, _)| *seq);
        MeasurementBatch {
            boot_id: self.boot_id,
            next_cursor,
            more: self.entries.len() > entries.len(),
            entries,
        }
    }
}

#[derive(Clone)]
pub struct Storage {
    dir: PathBuf,
}

impl Storage {
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn load<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let path = self.dir.join(name);
        let text = std::fs::read_to_string(&path).ok()?;
        match serde_json::from_str(&text) {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!(
                    "{} ist beschädigt und wird ignoriert: {err}",
                    path.display()
                );
                None
            }
        }
    }

    /// Schreibt atomar (temporäre Datei + Umbenennen), damit ein Stromausfall
    /// keine halbe Datei hinterlässt.
    pub fn save<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        let path = self.dir.join(name);
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autoweed_shared::{Event, EventKind};

    fn entry() -> BufferEntry {
        BufferEntry::Event(Event {
            at: chrono::Utc::now(),
            kind: EventKind::PumpUnlocked,
        })
    }

    #[test]
    fn cursor_acknowledges_entries() {
        let mut buffer = Buffer::new(7);
        for _ in 0..3 {
            buffer.push(entry());
        }
        let first = buffer.take(0, 0);
        assert_eq!(
            (first.entries.len(), first.next_cursor, first.more),
            (3, 3, false)
        );
        // Ohne Bestätigung kommen dieselben Einträge noch einmal.
        assert_eq!(buffer.take(0, 7).entries.len(), 3);
        buffer.push(entry());
        let second = buffer.take(first.next_cursor, 7);
        assert_eq!((second.entries.len(), second.next_cursor), (1, 4));
        // Nichts Neues: Cursor bleibt stehen.
        let empty = buffer.take(second.next_cursor, 7);
        assert_eq!((empty.entries.len(), empty.next_cursor), (0, 4));
    }

    #[test]
    fn foreign_boot_id_starts_from_scratch() {
        let mut buffer = Buffer::new(7);
        buffer.push(entry());
        buffer.push(entry());
        // Cursor eines früheren Starts darf nichts verwerfen.
        assert_eq!(buffer.take(2, 6).entries.len(), 2);
    }

    #[test]
    fn batches_are_limited() {
        let mut buffer = Buffer::new(1);
        for _ in 0..BATCH_LIMIT + 5 {
            buffer.push(entry());
        }
        let first = buffer.take(0, 1);
        assert_eq!(first.entries.len(), BATCH_LIMIT);
        assert!(first.more);
        let rest = buffer.take(first.next_cursor, 1);
        assert_eq!((rest.entries.len(), rest.more), (5, false));
    }
}
