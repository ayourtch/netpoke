mod state;
mod signaling;
mod webrtc_manager;

use axum::{Router, routing::{get, post}};
use std::net::SocketAddr;
use tower_http::{trace::TraceLayer, services::ServeDir};
use tracing_subscriber;
use state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let app_state = AppState::new();

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/signaling/start", post(signaling::signaling_start))
        .route("/api/signaling/ice", post(signaling::ice_candidate))
        .nest_service("/", ServeDir::new("server/static"))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}
