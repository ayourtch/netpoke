use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::state::{AppState, ClientSession};
use crate::webrtc_manager;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct SignalingStartRequest {
    pub sdp: String,
}

#[derive(Debug, Serialize)]
pub struct SignalingStartResponse {
    pub client_id: String,
    pub sdp: String,
}

#[derive(Debug, Deserialize)]
pub struct IceCandidateRequest {
    pub client_id: String,
    pub candidate: String,
}

pub async fn signaling_start(
    State(state): State<AppState>,
    Json(req): Json<SignalingStartRequest>,
) -> Result<Json<SignalingStartResponse>, StatusCode> {
    tracing::info!("Received signaling start request");

    // Create peer connection
    let peer = webrtc_manager::create_peer_connection()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create peer connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Handle offer and create answer
    let answer_sdp = webrtc_manager::handle_offer(&peer, req.sdp)
        .await
        .map_err(|e| {
            tracing::error!("Failed to handle offer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Store client session
    let client_id = uuid::Uuid::new_v4().to_string();
    let session = Arc::new(ClientSession {
        id: client_id.clone(),
        peer_connection: peer,
    });

    state.clients.write().await.insert(client_id.clone(), session);

    let response = SignalingStartResponse {
        client_id,
        sdp: answer_sdp,
    };

    Ok(Json(response))
}

pub async fn ice_candidate(
    State(state): State<AppState>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<StatusCode, StatusCode> {
    tracing::info!("Received ICE candidate for client {}", req.client_id);

    let clients = state.clients.read().await;
    let session = clients.get(&req.client_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Parse and add ICE candidate
    let candidate_init = serde_json::from_str(&req.candidate)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    session.peer_connection
        .add_ice_candidate(candidate_init)
        .await
        .map_err(|e| {
            tracing::error!("Failed to add ICE candidate: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signaling_request_deserialization() {
        let json = r#"{"sdp": "test-sdp"}"#;
        let req: SignalingStartRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.sdp, "test-sdp");
    }

    #[test]
    fn test_signaling_response_serialization() {
        let resp = SignalingStartResponse {
            client_id: "test-123".to_string(),
            sdp: "test-answer".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("test-123"));
    }
}
