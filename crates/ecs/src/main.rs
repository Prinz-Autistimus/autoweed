mod auth;
mod board;
mod influx;
mod ntfy;
mod poller;
mod routes;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use autoweed_shared::{BoardConfig, BoardStatus};
use axum::http::{HeaderValue, header};
use chrono::{DateTime, Utc};
use tokio::sync::Notify;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

/// Was ECS zuletzt vom Board weiß.
#[derive(Default)]
pub struct Live {
    pub board_reachable: bool,
    pub last_contact: Option<DateTime<Utc>>,
    pub board: Option<BoardStatus>,
}

pub struct Storage {
    config_file: PathBuf,
}

impl Storage {
    fn load_config(&self) -> Result<Option<BoardConfig>> {
        match std::fs::read_to_string(&self.config_file) {
            Ok(text) => Ok(Some(serde_json::from_str(&text).with_context(|| {
                format!("{} ist beschädigt", self.config_file.display())
            })?)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    /// Atomar schreiben, damit keine halbe Datei entsteht.
    pub fn save_config(&self, config: &BoardConfig) -> Result<()> {
        let tmp = self.config_file.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(config)?)?;
        std::fs::rename(&tmp, &self.config_file)?;
        Ok(())
    }
}

pub struct AppState {
    pub config: Mutex<BoardConfig>,
    pub live: Mutex<Live>,
    pub storage: Storage,
    pub board: board::BoardClient,
    pub influx: influx::Influx,
    pub ntfy: ntfy::Ntfy,
    pub auth: auth::Auth,
    /// Weckt den Poller, z. B. nach einer Konfigurationsänderung.
    pub config_changed: Notify,
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.into())
}

fn env_required(name: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => bail!("{name} fehlt"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let password = env_required("WEBUI_PASSWORD")?;
    if password.len() < 8 {
        bail!(
            "WEBUI_PASSWORD ist zu kurz (mindestens 8 Zeichen), die WebUI ist öffentlich erreichbar"
        );
    }

    let data_dir = PathBuf::from(env_or("DATA_DIR", "/data"));
    std::fs::create_dir_all(&data_dir)?;
    let storage = Storage {
        config_file: data_dir.join("config.json"),
    };
    let config = match storage.load_config()? {
        Some(config) => config,
        None => {
            let config = BoardConfig {
                version: 1,
                ..Default::default()
            };
            storage.save_config(&config)?;
            tracing::info!("Neue Konfiguration mit Standardwerten angelegt");
            config
        }
    };

    let state = Arc::new(AppState {
        config: Mutex::new(config),
        live: Mutex::new(Live::default()),
        storage,
        board: board::BoardClient::new(&env_required("BOARD_URL")?, &env_required("BOARD_TOKEN")?)?,
        influx: influx::Influx::new(
            &env_or("INFLUX_URL", "http://influxdb:8086"),
            &env_or("INFLUX_ORG", "autoweed"),
            &env_or("INFLUX_BUCKET", "autoweed"),
            &env_required("INFLUX_TOKEN")?,
        )?,
        ntfy: ntfy::Ntfy::new(
            &env_or("NTFY_URL", "http://host.docker.internal:8090"),
            &env_required("NTFY_TOPIC")?,
            std::env::var("PUBLIC_URL").ok().filter(|u| !u.is_empty()),
        )?,
        auth: auth::Auth::new(password),
        config_changed: Notify::new(),
    });

    let interval: u64 = env_or("POLL_INTERVAL_SECS", "10")
        .parse()
        .context("POLL_INTERVAL_SECS ist ungültig")?;
    tokio::spawn(poller::run(
        state.clone(),
        Duration::from_secs(interval.max(2)),
    ));

    // Gebautes Frontend; unbekannte Pfade bekommen index.html (Client-Routing).
    let static_dir = env_or("STATIC_DIR", "frontend/dist");
    let frontend = ServeDir::new(&static_dir)
        .not_found_service(ServeFile::new(format!("{static_dir}/index.html")));
    // Der Browser fragt jedes Mal nach, ob sich etwas geändert hat (ETag),
    // damit nach einem Update sofort die neue WebUI geladen wird.
    let frontend = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
        .service(frontend);
    let app = routes::router(state.clone())
        .fallback_service(frontend)
        .with_state(state);

    let addr = env_or("BIND_ADDR", "0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("ECS-Backend lauscht auf {addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// Docker stoppt Container mit SIGTERM, im Terminal kommt Ctrl+C.
async fn shutdown_signal() {
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        () = terminate => {},
    }
}
