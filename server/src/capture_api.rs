/// API handlers for packet capture functionality
///
/// These endpoints allow downloading captured packets as PCAP files
/// and viewing capture statistics.

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;
use serde::Deserialize;

use crate::packet_capture::{CaptureStats, PacketCaptureService};

/// Download captured packets as a PCAP file
pub async fn download_pcap(
    State(capture_service): State<Arc<PacketCaptureService>>,
) -> Response {
    if !capture_service.is_enabled() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Packet capture is not enabled"
            })),
        ).into_response();
    }

    let pcap_data = capture_service.generate_pcap();
    let stats = capture_service.stats();
    
    // Generate filename with timestamp
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("capture_{}.pcap", timestamp);

    tracing::info!(
        "PCAP download requested: {} packets, {} bytes",
        stats.packets_in_buffer,
        pcap_data.len()
    );

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.tcpdump.pcap"),
            (header::CONTENT_DISPOSITION, &format!("attachment; filename=\"{}\"", filename)),
        ],
        pcap_data,
    ).into_response()
}

/// Query parameters for survey-specific pcap download
#[derive(Deserialize)]
pub struct SurveySessionQuery {
    pub survey_session_id: String,
}

/// Download captured packets for a specific survey session as a PCAP file
pub async fn download_pcap_for_session(
    State(capture_service): State<Arc<PacketCaptureService>>,
    Query(query): Query<SurveySessionQuery>,
) -> Response {
    if !capture_service.is_enabled() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Packet capture is not enabled"
            })),
        ).into_response();
    }

    let survey_session_id = &query.survey_session_id;
    
    if survey_session_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "survey_session_id parameter is required"
            })),
        ).into_response();
    }

    let pcap_data = capture_service.generate_pcap_for_session(survey_session_id);
    
    // Count packets for logging
    let packet_count = capture_service.get_packets_for_session(survey_session_id).len();
    
    // Generate filename with timestamp and session ID (first 8 chars)
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let short_session_id = if survey_session_id.len() > 8 {
        &survey_session_id[..8]
    } else {
        survey_session_id
    };
    let filename = format!("capture_{}_{}.pcap", short_session_id, timestamp);

    tracing::info!(
        "Survey-specific PCAP download requested: session_id={}, {} packets, {} bytes",
        survey_session_id,
        packet_count,
        pcap_data.len()
    );

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.tcpdump.pcap"),
            (header::CONTENT_DISPOSITION, &format!("attachment; filename=\"{}\"", filename)),
        ],
        pcap_data,
    ).into_response()
}

/// Get capture statistics
pub async fn capture_stats(
    State(capture_service): State<Arc<PacketCaptureService>>,
) -> Json<CaptureStatsResponse> {
    let enabled = capture_service.is_enabled();
    let stats = if enabled {
        Some(capture_service.stats())
    } else {
        None
    };

    Json(CaptureStatsResponse {
        enabled,
        stats,
    })
}

/// Clear captured packets
pub async fn clear_capture(
    State(capture_service): State<Arc<PacketCaptureService>>,
) -> Response {
    if !capture_service.is_enabled() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Packet capture is not enabled"
            })),
        ).into_response();
    }

    capture_service.clear();
    tracing::info!("Capture buffer cleared");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "Capture buffer cleared"
        })),
    ).into_response()
}

#[derive(serde::Serialize)]
pub struct CaptureStatsResponse {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<CaptureStats>,
}
