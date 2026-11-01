mod api;
mod control;
mod hardware;
mod light;
mod state;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use autoweed_shared::{BoardConfig, BufferEntry, Event, EventKind};
use hardware::Hardware;
use hardware::pi::{ClimateSensor, PiHardware, Pins};
use hardware::sim::SimHardware;
use state::{Buffer, Shared, Storage};

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.into())
}

fn env_parse<T: std::str::FromStr>(name: &str, default: T) -> Result<T> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| anyhow::anyhow!("{name}={value} ist ungültig")),
        Err(_) => Ok(default),
    }
}

fn build_hardware() -> Result<Box<dyn Hardware>> {
    match env_or("HARDWARE", "pi").as_str() {
        "sim" => {
            let speed = env_parse("SIM_SPEED", 1.0)?;
            let fail_rate = env_parse("SIM_FAIL_RATE", 0.0)?;
            tracing::info!("Simulation aktiv (Geschwindigkeit {speed}x, Fehlerrate {fail_rate})");
            Ok(Box::new(SimHardware::new(speed, fail_rate)))
        }
        "pi" => {
            let sensor = match env_or("CLIMATE_SENSOR", "sht40").as_str() {
                "sht31" => ClimateSensor::Sht31,
                "sht40" => ClimateSensor::Sht40,
                other => bail!("CLIMATE_SENSOR={other} ist ungültig (sht31 oder sht40)"),
            };
            let pins = Pins {
                heater: env_parse("HEATER_PIN", 5)?,
                light: env_parse("LIGHT_PIN", 6)?,
                pump: env_parse("PUMP_PIN", 13)?,
            };
            let active_low = env_parse("RELAY_ACTIVE_LOW", true)?;
            tracing::info!("Hardware: {sensor:?}, {pins:?}, Relais aktiv-low: {active_low}");
            Ok(Box::new(PiHardware::new(sensor, pins, active_low)?))
        }
        other => bail!("HARDWARE={other} ist ungültig (pi oder sim)"),
    }
}

/// systemd-timesyncd legt diese Datei an, sobald die Uhr synchron ist.
/// Ohne Datei-Prüfung (TIME_SYNC_FILE leer) gilt die Uhr als synchron.
fn time_sync_check() -> Box<dyn Fn() -> bool + Send> {
    let file = env_or("TIME_SYNC_FILE", "/run/systemd/timesync/synchronized");
    if file.is_empty() {
        Box::new(|| true)
    } else {
        let path = PathBuf::from(file);
        Box::new(move || path.exists())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let token = std::env::var("BOARD_TOKEN").context("BOARD_TOKEN fehlt")?;
    if token.len() < 16 {
        bail!("BOARD_TOKEN ist zu kurz (mindestens 16 Zeichen)");
    }
    let storage = Storage::new(env_or("DATA_DIR", "/data"))?;
    let hardware = build_hardware()?;

    let saved: Option<BoardConfig> = storage.load(api::CONFIG_FILE);
    let config_from_ecs = saved.is_some();
    let config = saved.unwrap_or_default();
    tracing::info!(
        "Konfiguration: {}",
        if config_from_ecs {
            format!("v{} (gespeichert)", config.version)
        } else {
            "Standardwerte".into()
        }
    );

    let boot_id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as u64;
    let mut buffer = Buffer::new(boot_id);
    buffer.push(BufferEntry::Event(Event {
        at: chrono::Utc::now(),
        kind: EventKind::Started {
            version: env!("CARGO_PKG_VERSION").into(),
        },
    }));
    let shared = Arc::new(Mutex::new(Shared::new(config, config_from_ecs, buffer)));

    // Die Regelschleife läuft in einem eigenen Thread, unabhängig von der API.
    let stop = Arc::new(AtomicBool::new(false));
    let controller =
        control::Controller::new(hardware, shared.clone(), storage.clone(), time_sync_check());
    let control_thread = {
        let stop = stop.clone();
        std::thread::Builder::new()
            .name("regelung".into())
            .spawn(move || controller.run(stop))?
    };

    let app = api::router(api::AppState {
        shared,
        storage: Arc::new(storage),
        token: token.into(),
    });
    let addr = env_or("BIND_ADDR", "0.0.0.0:8080");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Board-API lauscht auf {addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    stop.store(true, Ordering::Relaxed);
    tokio::task::spawn_blocking(move || control_thread.join())
        .await?
        .map_err(|_| anyhow::anyhow!("Regelschleife ist abgestürzt"))?;
    Ok(())
}

/// Docker stoppt Container mit SIGTERM, im Terminal kommt Ctrl+C.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("Beende …");
}
