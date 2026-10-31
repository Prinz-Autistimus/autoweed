//! Holt periodisch Status und Messwerte vom Board, schickt geänderte
//! Konfigurationen und verschickt Benachrichtigungen.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use autoweed_shared::{BoardStatus, ErrorKind};
use chrono::{TimeDelta, Utc};

use crate::AppState;
use crate::ntfy::Level;

/// Sensorfehler werden gemeldet, wenn sie so lange bestehen (Back-off-Stufe 10 s).
const ERROR_NOTIFY_AFTER: TimeDelta = TimeDelta::seconds(10);
/// Board gilt nach so langer Funkstille als ausgefallen.
const UNREACHABLE_NOTIFY_AFTER: TimeDelta = TimeDelta::seconds(30);

struct PollerState {
    cursor: u64,
    boot_id: u64,
    unreachable_notified: bool,
    notified_errors: HashSet<ErrorKind>,
}

pub async fn run(state: Arc<AppState>, interval: Duration) {
    let mut poller = PollerState {
        cursor: 0,
        boot_id: 0,
        unreachable_notified: false,
        notified_errors: HashSet::new(),
    };
    loop {
        poll_once(&state, &mut poller).await;
        // Nach einer Konfigurationsänderung sofort wieder, sonst nach `interval`.
        tokio::select! {
            () = tokio::time::sleep(interval) => {},
            () = state.config_changed.notified() => {},
        }
    }
}

async fn poll_once(state: &AppState, poller: &mut PollerState) {
    let status = match state.board.status().await {
        Ok(status) => status,
        Err(err) => {
            board_unreachable(state, poller, &format!("{err:#}")).await;
            return;
        }
    };
    {
        let mut live = state.live.lock().unwrap();
        live.board_reachable = true;
        live.last_contact = Some(Utc::now());
        live.board = Some(status.clone());
    }
    if poller.unreachable_notified {
        poller.unreachable_notified = false;
        state
            .ntfy
            .send(
                Level::Resolved,
                "Board wieder erreichbar",
                "Die Verbindung zum Board steht wieder.",
            )
            .await;
    }

    push_config_if_needed(state, &status).await;
    notify_errors(state, poller, &status).await;
    collect_measurements(state, poller).await;
}

async fn board_unreachable(state: &AppState, poller: &mut PollerState, err: &str) {
    let last_contact = {
        let mut live = state.live.lock().unwrap();
        live.board_reachable = false;
        live.last_contact
    };
    tracing::warn!("{err}");
    let silent_for = last_contact.map_or(TimeDelta::MAX, |at| Utc::now() - at);
    if !poller.unreachable_notified && silent_for >= UNREACHABLE_NOTIFY_AFTER {
        poller.unreachable_notified = true;
        state
            .ntfy
            .send(
                Level::Problem,
                "Board nicht erreichbar",
                "ECS erreicht das Board nicht. Es regelt mit der letzten Konfiguration weiter, \
                 Messwerte werden gepuffert.",
            )
            .await;
    }
}

async fn push_config_if_needed(state: &AppState, status: &BoardStatus) {
    let config = state.config.lock().unwrap().clone();
    if status.config_version == Some(config.version) {
        return;
    }
    match state.board.put_config(&config).await {
        Ok(()) => tracing::info!("Konfiguration v{} an das Board geschickt", config.version),
        Err(err) => tracing::error!("Konfiguration konnte nicht gesendet werden: {err:#}"),
    }
}

fn describe(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::ClimateSensor => "Temperatur-/Feuchtesensor ausgefallen",
        ErrorKind::SoilSensor => "Bodenfeuchtesensor ausgefallen",
        ErrorKind::TimeSync => "Uhrzeit nicht synchronisiert",
        ErrorKind::BucketEmpty => "Eimer leer?",
        ErrorKind::NoConfig => "Keine Konfiguration",
        ErrorKind::Actuator => "Aktor-Fehler",
    }
}

async fn notify_errors(state: &AppState, poller: &mut PollerState, status: &BoardStatus) {
    let now = Utc::now();
    // NoConfig behebt ECS selbst, dafür gibt es keine Benachrichtigung.
    let active: Vec<_> = status
        .errors
        .iter()
        .filter(|e| e.kind != ErrorKind::NoConfig)
        .collect();
    for error in &active {
        if now - error.since >= ERROR_NOTIFY_AFTER && poller.notified_errors.insert(error.kind) {
            state
                .ntfy
                .send(Level::Problem, describe(error.kind), &error.message)
                .await;
        }
    }
    let cleared: Vec<_> = poller
        .notified_errors
        .iter()
        .copied()
        .filter(|kind| !active.iter().any(|e| e.kind == *kind))
        .collect();
    for kind in cleared {
        poller.notified_errors.remove(&kind);
        state
            .ntfy
            .send(
                Level::Resolved,
                &format!("Behoben: {}", describe(kind)),
                "Der Fehler besteht nicht mehr.",
            )
            .await;
    }
}

async fn collect_measurements(state: &AppState, poller: &mut PollerState) {
    loop {
        let batch = match state
            .board
            .measurements(poller.cursor, poller.boot_id)
            .await
        {
            Ok(batch) => batch,
            Err(err) => {
                tracing::error!("Messwerte konnten nicht abgerufen werden: {err:#}");
                return;
            }
        };
        // Erst nach erfolgreichem Schreiben bestätigen; sonst behält das Board die Daten.
        if let Err(err) = state.influx.write(&batch.entries).await {
            tracing::error!("Messwerte konnten nicht gespeichert werden: {err:#}");
            return;
        }
        poller.cursor = batch.next_cursor;
        poller.boot_id = batch.boot_id;
        if !batch.more {
            return;
        }
    }
}
