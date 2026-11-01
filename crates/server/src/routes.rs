//! REST-API für das Frontend.

use std::net::SocketAddr;
use std::sync::Arc;

use autoweed_shared::{
    ApiError, BoardConfig, Event, Health, History, LoginRequest, MAX_MANUAL_PUMP_SECS,
    MAX_OVERRIDE_MINUTES, OverrideRequest, PumpRequest, RangeQuery, ServerStatus, SessionInfo,
};
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::TimeDelta;

use crate::AppState;
use crate::auth::{self, LoginError};

/// Längster abfragbarer Zeitraum für Verlauf und Ereignisse.
const MAX_RANGE: TimeDelta = TimeDelta::days(400);

pub struct AppError(StatusCode, ApiError);

impl AppError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self(
            status,
            ApiError {
                message: message.into(),
                details: Vec::new(),
            },
        )
    }

    fn board(err: anyhow::Error) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, format!("{err:#}"))
    }

    fn internal(err: anyhow::Error) -> Self {
        tracing::error!("{err:#}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}"))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}

type ApiResult<T> = Result<T, AppError>;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let protected = Router::new()
        .route("/api/status", get(status))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/history", get(history))
        .route("/api/events", get(events))
        .route("/api/override/pump", post(pump))
        .route("/api/override/{actuator}", post(override_actuator))
        .route("/api/pump/unlock", post(unlock_pump))
        .route_layer(middleware::from_fn_with_state(state, require_session));
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/session", get(session))
        .merge(protected)
}

async fn require_session(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let valid = auth::session_cookie(request.headers()).is_some_and(|t| state.auth.is_valid(&t));
    if valid {
        next.run(request).await
    } else {
        AppError::new(StatusCode::UNAUTHORIZED, "Nicht angemeldet").into_response()
    }
}

async fn health() -> Json<Health> {
    Json(Health {
        service: "server".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> ApiResult<Response> {
    let ip = auth::client_ip(&headers, peer);
    match state.auth.login(&req.password, &ip) {
        Ok(token) => {
            let cookie = auth::set_cookie(&headers, &token, auth::SESSION_MAX_AGE);
            Ok((
                [(header::SET_COOKIE, cookie)],
                Json(SessionInfo {
                    authenticated: true,
                }),
            )
                .into_response())
        }
        Err(LoginError::WrongPassword) => {
            // Bremst automatisches Durchprobieren zusätzlich.
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            Err(AppError::new(StatusCode::UNAUTHORIZED, "Falsches Passwort"))
        }
        Err(LoginError::TooManyAttempts) => Err(AppError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "Zu viele Fehlversuche, bitte 15 Minuten warten",
        )),
    }
}

async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(token) = auth::session_cookie(&headers) {
        state.auth.logout(&token);
    }
    let cookie = auth::set_cookie(&headers, "", std::time::Duration::ZERO);
    (
        [(header::SET_COOKIE, cookie)],
        Json(SessionInfo {
            authenticated: false,
        }),
    )
        .into_response()
}

async fn session(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Json<SessionInfo> {
    Json(SessionInfo {
        authenticated: auth::session_cookie(&headers).is_some_and(|t| state.auth.is_valid(&t)),
    })
}

async fn status(State(state): State<Arc<AppState>>) -> Json<ServerStatus> {
    let config_version = state.config.lock().unwrap().version;
    let live = state.live.lock().unwrap();
    Json(ServerStatus {
        board_reachable: live.board_reachable,
        last_contact: live.last_contact,
        board: live.board.clone(),
        config_version,
        config_synced: live
            .board
            .as_ref()
            .is_some_and(|b| b.config_version == Some(config_version)),
    })
}

async fn get_config(State(state): State<Arc<AppState>>) -> Json<BoardConfig> {
    Json(state.config.lock().unwrap().clone())
}

/// Speichert die Konfiguration (Version wird hochgezählt) und stößt die
/// Übertragung ans Board sofort an.
async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(mut config): Json<BoardConfig>,
) -> ApiResult<Json<BoardConfig>> {
    if let Err(details) = config.validate() {
        return Err(AppError(
            StatusCode::UNPROCESSABLE_ENTITY,
            ApiError {
                message: "Konfiguration ist ungültig".into(),
                details,
            },
        ));
    }
    {
        let mut current = state.config.lock().unwrap();
        config.version = current.version + 1;
        state
            .storage
            .save_config(&config)
            .map_err(AppError::internal)?;
        *current = config.clone();
    }
    tracing::info!("Konfiguration v{} gespeichert", config.version);
    state.config_changed.notify_one();
    Ok(Json(config))
}

fn check_range(range: &RangeQuery) -> ApiResult<()> {
    if range.end <= range.start || range.end - range.start > MAX_RANGE {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "Ungültiger Zeitraum",
        ));
    }
    Ok(())
}

async fn history(
    State(state): State<Arc<AppState>>,
    Query(range): Query<RangeQuery>,
) -> ApiResult<Json<History>> {
    check_range(&range)?;
    let history = state
        .influx
        .history(range.start, range.end)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(history))
}

async fn events(
    State(state): State<Arc<AppState>>,
    Query(range): Query<RangeQuery>,
) -> ApiResult<Json<Vec<Event>>> {
    check_range(&range)?;
    let events = state
        .influx
        .events(range.start, range.end)
        .await
        .map_err(AppError::internal)?;
    Ok(Json(events))
}

async fn pump(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PumpRequest>,
) -> ApiResult<StatusCode> {
    if !(1..=MAX_MANUAL_PUMP_SECS).contains(&req.secs) {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            format!("Laufzeit muss zwischen 1 und {MAX_MANUAL_PUMP_SECS} s liegen"),
        ));
    }
    state.board.pump(&req).await.map_err(AppError::board)?;
    state.config_changed.notify_one();
    Ok(StatusCode::ACCEPTED)
}

async fn override_actuator(
    State(state): State<Arc<AppState>>,
    Path(actuator): Path<String>,
    Json(req): Json<OverrideRequest>,
) -> ApiResult<StatusCode> {
    if !matches!(actuator.as_str(), "heater" | "light") {
        return Err(AppError::new(StatusCode::NOT_FOUND, "Unbekannter Aktor"));
    }
    if req.minutes > MAX_OVERRIDE_MINUTES {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            format!("Höchstens {MAX_OVERRIDE_MINUTES} min"),
        ));
    }
    state
        .board
        .override_actuator(&actuator, &req)
        .await
        .map_err(AppError::board)?;
    state.config_changed.notify_one();
    Ok(StatusCode::ACCEPTED)
}

async fn unlock_pump(State(state): State<Arc<AppState>>) -> ApiResult<StatusCode> {
    state.board.unlock_pump().await.map_err(AppError::board)?;
    state.config_changed.notify_one();
    Ok(StatusCode::ACCEPTED)
}
