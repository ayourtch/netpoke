use axum::{
    extract::{State, ws::{WebSocket, WebSocketUpgrade, Message}},
    response::Response,
};
use futures::{stream::StreamExt, SinkExt};
use tokio::time::{interval, Duration};
use common::{DashboardMessage, ClientInfo};
use crate::state::AppState;

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

                // Extract peer address and port from the peer connection
                let mut peer_address = None;
                let mut peer_port = None;

                // For now, we'll set peer address to N/A since extracting it from ICE
                // candidates can be complex and error-prone. This can be enhanced later.
                peer_address = Some("N/A".to_string());
                peer_port = None;

                clients_info.push(ClientInfo {
                    id: session.id.clone(),
                    parent_id: session.parent_id.clone(),
                    ip_version: session.ip_version.clone(),
                    connected_at,
                    metrics,
                    peer_address,
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
