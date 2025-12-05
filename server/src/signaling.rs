use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use crate::state::AppState;

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

    // For now, just echo back a dummy SDP answer
    // We'll implement real WebRTC in next task
    let client_id = uuid::Uuid::new_v4().to_string();

    let response = SignalingStartResponse {
        client_id,
        sdp: "dummy-answer-sdp".to_string(),
    };

    Ok(Json(response))
}

pub async fn ice_candidate(
    State(state): State<AppState>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<StatusCode, StatusCode> {
    tracing::info!("Received ICE candidate for client {}", req.client_id);

    // Store ICE candidate (will implement properly in next task)

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
