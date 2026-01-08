/// API handlers for DTLS keylog functionality
///
/// These endpoints allow downloading DTLS keys for decryption in Wireshark.
/// Keys are provided in the SSLKEYLOGFILE format.
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::dtls_keylog::{DtlsKeylogService, KeylogStats};

/// Query parameters for session-specific keylog download
#[derive(Deserialize)]
pub struct KeylogSessionQuery {
    pub survey_session_id: String,
}

/// Download DTLS keylog for a specific survey session
pub async fn download_keylog_for_session(
    State(keylog_service): State<Arc<DtlsKeylogService>>,
    Query(query): Query<KeylogSessionQuery>,
) -> Response {
    if !keylog_service.is_enabled() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "DTLS keylog storage is not enabled"
            })),
        )
            .into_response();
    }

    let survey_session_id = &query.survey_session_id;

    if survey_session_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "survey_session_id parameter is required"
            })),
        )
            .into_response();
    }

    let keylog_content = keylog_service.generate_keylog_file(survey_session_id);

    if keylog_content.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "No DTLS keys found for this session"
            })),
        )
            .into_response();
    }

    // Count entries for logging
    let entry_count = keylog_content.lines().count();

    // Generate filename with timestamp and session ID (first 8 chars)
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let short_session_id = if survey_session_id.len() > 8 {
        &survey_session_id[..8]
    } else {
        survey_session_id
    };
    let filename = format!("dtls_keys_{}_{}.log", short_session_id, timestamp);

    tracing::info!(
        "DTLS keylog download requested: session_id={}, {} entries, {} bytes",
        survey_session_id,
        entry_count,
        keylog_content.len()
    );

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        keylog_content,
    )
        .into_response()
}

/// Get keylog statistics
pub async fn keylog_stats(
    State(keylog_service): State<Arc<DtlsKeylogService>>,
) -> Json<KeylogStatsResponse> {
    let enabled = keylog_service.is_enabled();
    let stats = if enabled {
        Some(keylog_service.stats())
    } else {
        None
    };

    Json(KeylogStatsResponse { enabled, stats })
}

/// Clear all stored keylogs
pub async fn clear_keylog(State(keylog_service): State<Arc<DtlsKeylogService>>) -> Response {
    if !keylog_service.is_enabled() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "DTLS keylog storage is not enabled"
            })),
        )
            .into_response();
    }

    keylog_service.clear();
    tracing::info!("DTLS keylog storage cleared");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "message": "DTLS keylog storage cleared"
        })),
    )
        .into_response()
}

#[derive(serde::Serialize)]
pub struct KeylogStatsResponse {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<KeylogStats>,
}
