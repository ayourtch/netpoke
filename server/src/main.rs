mod state;
mod signaling;
mod webrtc_manager;
mod data_channels;
mod measurements;
mod dashboard;

use axum::{Router, routing::{get, post}, extract::State, Json};
use std::net::SocketAddr;
use tower_http::{trace::TraceLayer, services::ServeDir};
use tracing_subscriber;
use state::AppState;
use common::{DashboardMessage, ClientInfo};

use axum::{http::uri::Uri, response::Redirect};
use axum_server::tls_rustls::RustlsConfig;

use axum::routing::IntoMakeService;


fn get_make_service() -> IntoMakeService<axum::Router> {
    let app_state = AppState::new();
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/signaling/start", post(signaling::signaling_start))
        .route("/api/signaling/ice", post(signaling::ice_candidate))
        .route("/api/signaling/ice/remote", post(signaling::get_ice_candidates))
        .route("/api/dashboard/ws", get(dashboard::dashboard_ws_handler))
        .route("/api/dashboard/debug", get(dashboard_debug))
        .nest_service("/", ServeDir::new("server/static"))
        .with_state(app_state)
        .layer(TraceLayer::new_for_http());

    let srv = app.into_make_service();
    srv
}

async fn http_server() {
    // let app = Router::new().route("/", get(http_handler));

    let srv = get_make_service();

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("http listening on {}", addr);
    axum_server::bind(addr)
        .serve(srv)
        .await
        .unwrap();
}

async fn http_handler(uri: Uri) -> Redirect {
    let uri = format!("https://127.0.0.1:3443{}", uri.path());

    Redirect::temporary(&uri)
}

async fn https_server() {
    let srv = get_make_service();

    let config = RustlsConfig::from_pem_file(
        "server.crt",
        "server.key",
    )
    .await
    .unwrap();

    let addr = SocketAddr::from(([0, 0, 0, 0], 3443));
    println!("https listening on {}", addr);
    axum_server::bind_rustls(addr, config)
        .serve(srv)
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
    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

async fn dashboard_debug(State(state): State<AppState>) -> Json<DashboardMessage> {
    let clients_lock = state.clients.read().await;
    let mut clients_info = Vec::new();

    for (_, session) in clients_lock.iter() {
        let metrics = session.metrics.read().await.clone();
        let connected_at = session.connected_at.elapsed().as_secs();
        let measurement_state = session.measurement_state.read().await;
        let current_seq = measurement_state.probe_seq;

        clients_info.push(ClientInfo {
            id: session.id.clone(),
            parent_id: session.parent_id.clone(),
            ip_version: session.ip_version.clone(),
            connected_at,
            metrics,
            peer_address: Some("N/A".to_string()),
            peer_port: None,
            current_seq,
        });
    }
    drop(clients_lock);

    Json(DashboardMessage {
        clients: clients_info,
    })
}
