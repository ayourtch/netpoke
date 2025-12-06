mod state;
mod signaling;
mod webrtc_manager;
mod data_channels;
mod measurements;
mod dashboard;

use axum::{Router, routing::{get, post}};
use std::net::SocketAddr;
use tower_http::{trace::TraceLayer, services::ServeDir};
use tracing_subscriber;
use state::AppState;

use axum::{http::uri::Uri, response::Redirect};
use axum_server::tls_rustls::RustlsConfig;

async fn http_server() {
    // let app = Router::new().route("/", get(http_handler));

    let app_state = AppState::new();
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/signaling/start", post(signaling::signaling_start))
        .route("/api/signaling/ice", post(signaling::ice_candidate))
        .route("/api/dashboard/ws", get(dashboard::dashboard_ws_handler))
        .nest_service("/", ServeDir::new("server/static"))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http());


    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("http listening on {}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn http_handler(uri: Uri) -> Redirect {
    let uri = format!("https://127.0.0.1:3443{}", uri.path());

    Redirect::temporary(&uri)
}

async fn https_server() {
    let app_state = AppState::new();
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/signaling/start", post(signaling::signaling_start))
        .route("/api/signaling/ice", post(signaling::ice_candidate))
        .route("/api/dashboard/ws", get(dashboard::dashboard_ws_handler))
        .nest_service("/", ServeDir::new("server/static"))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http());

    let config = RustlsConfig::from_pem_file(
        "server.crt",
        "server.key",
    )
    .await
    .unwrap();

    let addr = SocketAddr::from(([0, 0, 0, 0], 3443));
    println!("https listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let http = tokio::spawn(http_server());
    let https = tokio::spawn(https_server());

    // Ignore errors.
    let _ = tokio::join!(http, https);

/*
    let app_state = AppState::new();
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/signaling/start", post(signaling::signaling_start))
        .route("/api/signaling/ice", post(signaling::ice_candidate))
        .route("/api/dashboard/ws", get(dashboard::dashboard_ws_handler))
        .nest_service("/", ServeDir::new("server/static"))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
*/
    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}
