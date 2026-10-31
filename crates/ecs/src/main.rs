use autoweed_shared::Health;
use axum::{Json, Router, routing::get};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/api/v1/health", get(health));

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("ECS-Backend lauscht auf {addr}");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<Health> {
    Json(Health {
        service: "ecs".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}
