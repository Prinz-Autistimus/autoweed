//! HTTP-API des Boards. Alles außer `/api/v1/health` braucht den Bearer-Token.

use std::sync::{Arc, Mutex};

use autoweed_shared::BufferEntry;
use autoweed_shared::{
    Actuator, BoardConfig, BoardStatus, Event, EventKind, Health, MeasurementBatch,
    MeasurementsQuery, OverrideRequest, PumpRequest,
};
use axum::extract::{Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::state::{Command, Shared, Storage};

pub const CONFIG_FILE: &str = "config.json";

#[derive(Clone)]
pub struct AppState {
    pub shared: Arc<Mutex<Shared>>,
    pub storage: Arc<Storage>,
    pub token: Arc<str>,
}

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/measurements", get(measurements))
        .route("/api/v1/config", get(get_config).put(put_config))
        .route("/api/v1/override/pump", post(pump))
        .route("/api/v1/override/{actuator}", post(override_actuator))
        .route("/api/v1/pump/unlock", post(unlock_pump))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth));
    Router::new()
        .route("/api/v1/health", get(health))
        .merge(protected)
        .with_state(state)
}

/// Vergleich in konstanter Zeit, damit die Antwortzeit nichts über den Token verrät.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if constant_time_eq(provided.as_bytes(), state.token.as_bytes()) {
        next.run(request).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

async fn health() -> Json<Health> {
    Json(Health {
        service: "board".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

async fn status(State(state): State<AppState>) -> Json<BoardStatus> {
    Json(state.shared.lock().unwrap().status.clone())
}

async fn measurements(
    State(state): State<AppState>,
    Query(query): Query<MeasurementsQuery>,
) -> Json<MeasurementBatch> {
    let mut shared = state.shared.lock().unwrap();
    Json(shared.buffer.take(query.since, query.boot_id))
}

async fn get_config(State(state): State<AppState>) -> Json<BoardConfig> {
    Json(state.shared.lock().unwrap().config.clone())
}

async fn put_config(
    State(state): State<AppState>,
    Json(config): Json<BoardConfig>,
) -> Result<StatusCode, (StatusCode, Json<Vec<String>>)> {
    config
        .validate()
        .map_err(|errors| (StatusCode::UNPROCESSABLE_ENTITY, Json(errors)))?;
    state.storage.save(CONFIG_FILE, &config).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(vec![format!("Speichern fehlgeschlagen: {err:#}")]),
        )
    })?;
    let mut shared = state.shared.lock().unwrap();
    let version = config.version;
    shared.config = config;
    shared.config_from_ecs = true;
    shared.buffer.push(BufferEntry::Event(Event {
        at: chrono::Utc::now(),
        kind: EventKind::ConfigApplied { version },
    }));
    tracing::info!("Konfiguration v{version} übernommen");
    Ok(StatusCode::NO_CONTENT)
}

async fn pump(State(state): State<AppState>, Json(req): Json<PumpRequest>) -> StatusCode {
    command(&state, Command::Pump { secs: req.secs })
}

async fn override_actuator(
    State(state): State<AppState>,
    Path(actuator): Path<String>,
    Json(req): Json<OverrideRequest>,
) -> StatusCode {
    let actuator = match actuator.as_str() {
        "heater" => Actuator::Heater,
        "light" => Actuator::Light,
        _ => return StatusCode::NOT_FOUND,
    };
    command(&state, Command::Override(actuator, req))
}

async fn unlock_pump(State(state): State<AppState>) -> StatusCode {
    command(&state, Command::UnlockPump)
}

fn command(state: &AppState, command: Command) -> StatusCode {
    state.shared.lock().unwrap().commands.push(command);
    StatusCode::ACCEPTED
}
