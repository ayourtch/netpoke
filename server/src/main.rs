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
use webrtc::stats::StatsReportType;
use webrtc::ice::candidate::CandidatePairState;

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

        // Try to get the actual peer (client) address from the selected candidate pair
        let mut peer_address = None;
        let mut peer_port = None;

        // Check connection state before trying to get stats
        use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
        let conn_state = session.peer_connection.connection_state();

        tracing::debug!("Client {} connection state: {:?}", session.id, conn_state);

        // Only try to get stats if connection is established
        if conn_state == RTCPeerConnectionState::Connected {
            let stats_report = session.peer_connection.get_stats().await;
            tracing::debug!("Client {} stats report has {} entries", session.id, stats_report.reports.len());

            // Find the selected candidate pair
            for stat in stats_report.reports.values() {
                if let StatsReportType::CandidatePair(pair) = stat {
                    tracing::debug!("Candidate pair state: {:?}, nominated: {}", pair.state, pair.nominated);

                    // Look for succeeded or in-progress pairs
                    if pair.state == CandidatePairState::Succeeded ||
                       pair.state == CandidatePairState::InProgress && pair.nominated {
                        // Find the remote candidate by ID to get the IP address
                        for candidate_stat in stats_report.reports.values() {
                            if let StatsReportType::RemoteCandidate(candidate) = candidate_stat {
                                if candidate.id == pair.remote_candidate_id {
                                    peer_address = Some(candidate.ip.clone());
                                    peer_port = Some(candidate.port);
                                    tracing::info!("Got client {} address from selected pair: {}:{}",
                                        session.id, candidate.ip, candidate.port);
                                    break;
                                }
                            }
                        }
                        if peer_address.is_some() {
                            break;
                        }
                    }
                }
            }
        }

        // Fallback to stored address if stats didn't work
        if peer_address.is_none() {
            let stored_peer = session.peer_address.lock().await;
            if let Some((addr, port)) = stored_peer.as_ref() {
                peer_address = Some(addr.clone());
                peer_port = Some(*port);
                tracing::debug!("Using stored peer address for client {}: {}:{}", session.id, addr, port);
            }
        }

        let peer_address_final = peer_address.unwrap_or_else(|| {
            tracing::warn!("No peer address available for client {}", session.id);
            "N/A".to_string()
        });

        clients_info.push(ClientInfo {
            id: session.id.clone(),
            parent_id: session.parent_id.clone(),
            ip_version: session.ip_version.clone(),
            connected_at,
            metrics,
            peer_address: Some(peer_address_final),
            peer_port,
            current_seq,
        });
    }
    drop(clients_lock);

    Json(DashboardMessage {
        clients: clients_info,
    })
}
