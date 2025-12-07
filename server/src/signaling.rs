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
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

/// Determine if an ICE candidate is IPv4 or IPv6 by parsing the candidate string
/// Returns Some("ipv4"), Some("ipv6"), or None if unable to determine
fn get_candidate_ip_version(candidate_str: &str) -> Option<String> {
    // Parse the candidate SDP attribute
    // Format: "candidate:foundation component protocol priority ip port typ type ..."
    // Example: "candidate:1234567890 1 udp 2122260223 192.168.1.100 54321 typ host"

    if let Some(candidate_part) = candidate_str.strip_prefix("candidate:") {
        let parts: Vec<&str> = candidate_part.split_whitespace().collect();
        if parts.len() >= 5 {
            let ip = parts[4]; // IP address is the 5th field (index 4)

            // Check if it contains ':' which indicates IPv6
            if ip.contains(':') {
                return Some("ipv6".to_string());
            } else if ip.contains('.') {
                return Some("ipv4".to_string());
            }
        }
    }

    None
}

/// Filter SDP to remove ICE candidates of the wrong IP version
fn filter_sdp_candidates(sdp: &str, ip_version: Option<&String>) -> String {
    if ip_version.is_none() {
        return sdp.to_string();
    }

    let expected_version = ip_version.unwrap();
    let mut filtered_lines = Vec::new();

    for line in sdp.lines() {
        if line.starts_with("a=candidate:") {
            // Parse the candidate line
            // Format: a=candidate:foundation component protocol priority ip port typ type ...
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let ip = parts[4]; // IP address is the 5th field (index 4)

                let detected_version = if ip.contains(':') {
                    "ipv6"
                } else if ip.contains('.') {
                    "ipv4"
                } else {
                    ""
                };

                if !detected_version.is_empty() && detected_version.eq_ignore_ascii_case(expected_version) {
                    // Keep this candidate
                    filtered_lines.push(line);
                    tracing::debug!("Keeping {} candidate in SDP: {}", detected_version, ip);
                } else if !detected_version.is_empty() {
                    // Filter out this candidate
                    tracing::debug!("Filtering out {} candidate from SDP for {} connection: {}",
                        detected_version, expected_version, ip);
                } else {
                    // Unknown format, keep it
                    filtered_lines.push(line);
                }
            } else {
                // Malformed candidate line, keep it
                filtered_lines.push(line);
            }
        } else {
            // Not a candidate line, keep it
            filtered_lines.push(line);
        }
    }

    filtered_lines.join("\r\n") + "\r\n"
}

#[derive(Debug, Deserialize)]
pub struct SignalingStartRequest {
    pub sdp: String,
    pub parent_client_id: Option<String>, // For grouping multiple sessions
    pub ip_version: Option<String>,       // "ipv4" or "ipv6"
}

#[derive(Debug, Serialize)]
pub struct SignalingStartResponse {
    pub client_id: String,
    pub parent_client_id: Option<String>,
    pub ip_version: Option<String>,
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
    tracing::info!("IP Version: {:?}", req.ip_version);

    // Create peer connection
    let peer = webrtc_manager::create_peer_connection()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create peer connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let client_id = uuid::Uuid::new_v4().to_string();

    // Use provided parent_client_id or create a new one
    let parent_client_id = req.parent_client_id.unwrap_or_else(|| client_id.clone());

    let data_channels = Arc::new(tokio::sync::RwLock::new(crate::state::DataChannels::new()));
    let metrics = Arc::new(tokio::sync::RwLock::new(common::ClientMetrics::default()));
    let measurement_state = Arc::new(tokio::sync::RwLock::new(crate::state::MeasurementState::new()));
    let ice_candidates = Arc::new(tokio::sync::Mutex::new(std::collections::VecDeque::new()));
    let peer_address = Arc::new(tokio::sync::Mutex::new(None::<(String, u16)>));

    let session = Arc::new(crate::state::ClientSession {
        id: client_id.clone(),
        parent_id: Some(parent_client_id.clone()),
        ip_version: req.ip_version.clone(),
        peer_connection: peer.clone(),
        data_channels,
        metrics,
        measurement_state,
        connected_at: std::time::Instant::now(),
        ice_candidates: ice_candidates.clone(),
        peer_address: peer_address.clone(),
    });

    // Set up data channel handlers
    data_channels::setup_data_channel_handlers(&peer, session.clone()).await;

    // Set up ICE candidate handler to send candidates back to client
    let client_id_for_ice = client_id.clone();
    let ice_candidates_for_handler = session.ice_candidates.clone();
    let ip_version_for_filter = req.ip_version.clone();
    peer.on_ice_candidate(Box::new(move |candidate| {
        if let Some(c) = candidate {
            tracing::info!("Server ICE candidate gathered for client {}", client_id_for_ice);

            // Extract the IP address from the candidate
            let candidate_address = c.address.clone();

            if let Ok(candidate_json) = serde_json::to_string(&c) {
                let candidates = ice_candidates_for_handler.clone();
                let ip_version_filter = ip_version_for_filter.clone();
                let client_id = client_id_for_ice.clone();

                tokio::spawn(async move {
                    // Filter candidates by IP version if specified
                    let should_store = if let Some(ref expected_version) = ip_version_filter {
                        // Check if address is IPv4 or IPv6
                        let detected_version = if candidate_address.contains(':') {
                            "ipv6"
                        } else if candidate_address.contains('.') {
                            "ipv4"
                        } else {
                            ""
                        };

                        if !detected_version.is_empty() {
                            let matches = detected_version.eq_ignore_ascii_case(expected_version);
                            if matches {
                                tracing::info!("Server storing {} candidate for client {}: {}",
                                    detected_version, client_id, candidate_address);
                            } else {
                                tracing::debug!("Server filtering out {} candidate for {} connection (client {}): {}",
                                    detected_version, expected_version, client_id, candidate_address);
                            }
                            matches
                        } else {
                            // Unable to determine version, store it anyway
                            tracing::debug!("Unable to determine server candidate IP version for client {}, storing anyway: {}",
                                client_id, candidate_address);
                            true
                        }
                    } else {
                        // No IP version filter specified, store all candidates
                        tracing::debug!("No IP version filter for client {}, storing candidate: {}",
                            client_id, candidate_address);
                        true
                    };

                    if should_store {
                        let mut candidates = candidates.lock().await;
                        candidates.push_back(candidate_json);
                        tracing::debug!("Stored ICE candidate in VecDeque (total: {})", candidates.len());
                    }

                    // Note: We DON'T extract peer address from server's own ICE candidates.
                    // The peer address comes from the remote (client) candidate, which we'll
                    // get from WebRTC stats once the connection is established.
                });
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

    // Filter SDP candidates based on IP version
    let filtered_sdp = filter_sdp_candidates(&answer_sdp, req.ip_version.as_ref());
    tracing::info!("Filtered SDP for {:?} connection, original candidates: {}, filtered candidates: {}",
        req.ip_version,
        answer_sdp.matches("a=candidate:").count(),
        filtered_sdp.matches("a=candidate:").count());

    state.clients.write().await.insert(client_id.clone(), session);

    let response = SignalingStartResponse {
        client_id,
        parent_client_id: Some(parent_client_id),
        ip_version: req.ip_version,
        sdp: filtered_sdp,
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

    // Filter candidate by IP version if session has one specified
    let should_add = if let Some(ref expected_version) = session.ip_version {
        let candidate_str = &candidate_init.candidate;
        if let Some(detected_version) = get_candidate_ip_version(candidate_str) {
            let matches = detected_version.eq_ignore_ascii_case(expected_version);
            if matches {
                tracing::info!("Server accepting {} candidate from client {}: {}",
                    detected_version, req.client_id, candidate_str);
            } else {
                tracing::info!("Server rejecting {} candidate for {} connection (client {}): {}",
                    detected_version, expected_version, req.client_id, candidate_str);
            }
            matches
        } else {
            // Unable to determine version, accept it anyway
            tracing::debug!("Unable to determine client candidate IP version for client {}, accepting anyway: {}",
                req.client_id, candidate_str);
            true
        }
    } else {
        // No IP version filter specified, accept all candidates
        true
    };

    if should_add {
        session.peer_connection
            .add_ice_candidate(candidate_init)
            .await
            .map_err(|e| {
                tracing::error!("Failed to add ICE candidate: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        Ok(StatusCode::OK)
    } else {
        // Return OK even though we filtered it out, to avoid breaking client flow
        tracing::debug!("Filtered out candidate for client {}, returning OK", req.client_id);
        Ok(StatusCode::OK)
    }
}

pub async fn get_ice_candidates(
    State(state): State<AppState>,
    Json(req): Json<IceCandidateRequest>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let clients = state.clients.read().await;
    let session = clients.get(&req.client_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Drain all pending ICE candidates
    let mut candidates = session.ice_candidates.lock().await;
    let ice_candidates: Vec<String> = candidates.drain(..).collect();

    tracing::debug!("Returning {} ICE candidates to client {}", ice_candidates.len(), req.client_id);

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
