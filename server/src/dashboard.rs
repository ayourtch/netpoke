use axum::{
    extract::{State, ws::{WebSocket, WebSocketUpgrade, Message}},
    response::Response,
};
use futures::{stream::StreamExt, SinkExt};
use tokio::time::{interval, Duration};
use common::{DashboardMessage, ClientInfo};
use crate::state::AppState;
use webrtc::stats::StatsReportType;
use webrtc::ice::candidate::CandidatePairState;

pub async fn dashboard_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| dashboard_ws(socket, state))
}

async fn dashboard_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Spawn task to send updates periodically
    let send_task = tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            // Collect all client metrics
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

                // Try to get peer info from ICE stats (the selected candidate pair)
                let stats_report = session.peer_connection.get_stats().await;
                for stat in stats_report.reports.values() {
                    if let StatsReportType::CandidatePair(pair) = stat {
                        // Check if this pair is selected/nominated
                        if pair.nominated || pair.state == CandidatePairState::Succeeded {
                            // Find the remote candidate by ID to get the IP address
                            for candidate_stat in stats_report.reports.values() {
                                if let StatsReportType::RemoteCandidate(candidate) = candidate_stat {
                                    if candidate.id == pair.remote_candidate_id {
                                        peer_address = Some(candidate.ip.clone());
                                        peer_port = Some(candidate.port);
                                        tracing::debug!("Got selected candidate pair: remote={}:{}", candidate.ip, candidate.port);
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

                // Fallback to stored address if stats didn't work
                if peer_address.is_none() {
                    let stored_peer = session.peer_address.lock().await;
                    if let Some((addr, port)) = stored_peer.as_ref() {
                        peer_address = Some(addr.clone());
                        peer_port = Some(*port);
                    }
                }

                let peer_address_final = peer_address.unwrap_or_else(|| "N/A".to_string());

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

            let msg = DashboardMessage {
                clients: clients_info,
            };

            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Receive task (handle incoming messages if needed)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // Dashboard doesn't send messages for now
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}
