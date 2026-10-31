use autoweed_shared::Health;
use axum::{Json, Router, routing::get};
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Gebautes Frontend; unbekannte Pfade bekommen index.html (Client-Routing).
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "frontend/dist".into());
    let frontend = ServeDir::new(&static_dir)
        .not_found_service(ServeFile::new(format!("{static_dir}/index.html")));

    let app = Router::new()
        .route("/api/v1/health", get(health))
        .fallback_service(frontend);

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
