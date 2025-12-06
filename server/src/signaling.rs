use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::webrtc_manager;
use crate::data_channels;
use std::sync::Arc;
use tokio::sync::broadcast;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

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
    tracing::info!("Received signaling start request: {:?}", &req);
    tracing::info!("SDP: {}", &req.sdp);

    // Create peer connection
    let peer = webrtc_manager::create_peer_connection()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create peer connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let client_id = uuid::Uuid::new_v4().to_string();
    let data_channels = Arc::new(tokio::sync::RwLock::new(crate::state::DataChannels::new()));
    let metrics = Arc::new(tokio::sync::RwLock::new(common::ClientMetrics::default()));
    let measurement_state = Arc::new(tokio::sync::RwLock::new(crate::state::MeasurementState::new()));
    let (ice_candidate_tx, _) = broadcast::channel(100);

    let session = Arc::new(crate::state::ClientSession {
        id: client_id.clone(),
        peer_connection: peer.clone(),
        data_channels,
        metrics,
        measurement_state,
        connected_at: std::time::Instant::now(),
        ice_candidate_tx: ice_candidate_tx.clone(),
    });

    // Set up data channel handlers
    data_channels::setup_data_channel_handlers(&peer, session.clone()).await;

    // Set up ICE candidate handler to send candidates back to client
    let client_id_for_ice = client_id.clone();
    let ice_candidate_tx_for_handler = ice_candidate_tx.clone();
    peer.on_ice_candidate(Box::new(move |candidate| {
        if let Some(c) = candidate {
            tracing::info!("Server ICE candidate gathered for client {}", client_id_for_ice);
            if let Ok(candidate_json) = serde_json::to_string(&c) {
                let _ = ice_candidate_tx_for_handler.send(candidate_json);
            }
        }
        Box::pin(async {})
    }));

    // Handle offer and create answer
    let answer_sdp = webrtc_manager::handle_offer(&peer, req.sdp)
        .await
        .map_err(|e| {
            tracing::error!("Failed to handle offer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    state.clients.write().await.insert(client_id.clone(), session);

    let response = SignalingStartResponse {
        client_id,
        sdp: answer_sdp,
    };
    tracing::info!("RESPONSE: {:?}", &response);

    Ok(Json(response))
}

pub async fn ice_candidate(
    State(state): State<AppState>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<StatusCode, StatusCode> {
    tracing::info!("Received ICE candidate for client {}", req.client_id);
    tracing::debug!("ICE candidate data: {}", &req.candidate);

    let clients = state.clients.read().await;
    let session = clients.get(&req.client_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Parse and add ICE candidate
    let candidate_init: RTCIceCandidateInit = match serde_json::from_str(&req.candidate) {
        Ok(val) => val,
        Err(e) => {
            tracing::error!("Failed to parse ICE candidate JSON: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    session.peer_connection
        .add_ice_candidate(candidate_init)
        .await
        .map_err(|e| {
            tracing::error!("Failed to add ICE candidate: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::OK)
}

pub async fn get_ice_candidates(
    State(state): State<AppState>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let clients = state.clients.read().await;
    let session = clients.get(&req.client_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut ice_candidates = Vec::new();
    let mut rx = session.ice_candidate_tx.subscribe();

    // Try to receive any pending ICE candidates (non-blocking first)
    while let Ok(candidate) = rx.try_recv() {
        ice_candidates.push(candidate);
    }

    // Then try to receive with a timeout to catch any candidates that arrive
    if let Ok(candidate) = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        rx.recv()
    ).await {
        if let Ok(c) = candidate {
            ice_candidates.push(c);
        }
    }

    Ok(Json(ice_candidates))
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
