use autoweed_shared::Health;
use axum::{Json, Router, routing::get};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/api/v1/health", get(health));

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Board-API lauscht auf {addr}");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<Health> {
    Json(Health {
        service: "board".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}
