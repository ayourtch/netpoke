use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;
use std::collections::HashSet;
use crate::state::AppState;

#[derive(Serialize)]
struct CleanupResponse {
    removed: Vec<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn cleanup_client_handler(
    Path(client_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    tracing::info!("Cleanup request for client: {}", client_id);

    let mut clients = state.clients.write().await;

    tracing::info!("Current clients: {:?}", clients.keys().collect::<Vec<_>>());

    // Check if client exists
    if !clients.contains_key(&client_id) {
        tracing::warn!("Client {} not found in state", client_id);
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Client not found".to_string(),
            }),
        )
            .into_response();
    }

    tracing::info!("Client {} found, proceeding with cleanup", client_id);

    // Find all descendants recursively
    let mut to_remove = HashSet::new();
    to_remove.insert(client_id.clone());

    let mut changed = true;
    while changed {
        changed = false;
        for (id, session) in clients.iter() {
            if let Some(parent_id) = &session.parent_id {
                if to_remove.contains(parent_id) && !to_remove.contains(id) {
                    to_remove.insert(id.clone());
                    changed = true;
                }
            }
        }
    }

    // Close WebRTC connections and remove all clients in the deletion set
    let mut removed = Vec::new();
    for id in &to_remove {
        if let Some(session) = clients.get(id) {
            // Close the WebRTC peer connection to clean up resources
            if let Err(e) = session.peer_connection.close().await {
                tracing::warn!("Error closing peer connection for {}: {}", id, e);
            } else {
                tracing::info!("Closed peer connection for {}", id);
            }
        }
    }

    // Now remove from the HashMap
    for id in to_remove {
        clients.remove(&id);
        removed.push(id);
    }

    tracing::info!("Removed {} clients: {:?}", removed.len(), removed);

    (StatusCode::OK, Json(CleanupResponse { removed })).into_response()
}
