//! Analyst API for browsing survey data
//!
//! Provides endpoints for analysts to list and view survey sessions, recordings,
//! and metrics for analysis and export.
//! Access is controlled via the `[analyst_access]` configuration which maps
//! usernames to lists of magic keys they can view. Use `["*"]` for wildcard access.

use crate::database::DbConnection;
use crate::dtls_keylog::DtlsKeylogService;
use crate::packet_capture::PacketCaptureService;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use netpoke_auth::SessionData;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// State shared by analyst API handlers
#[derive(Clone)]
pub struct AnalystState {
    pub db: DbConnection,
    pub analyst_access: HashMap<String, Vec<String>>,
    pub capture_service: Option<Arc<PacketCaptureService>>,
    pub keylog_service: Option<Arc<DtlsKeylogService>>,
}

/// Check if a user has access to a specific magic key
fn user_has_access(analyst_access: &HashMap<String, Vec<String>>, username: &str, magic_key: &str) -> bool {
    if let Some(allowed_keys) = analyst_access.get(username) {
        allowed_keys.iter().any(|k| k == "*" || k == magic_key)
    } else {
        false
    }
}

/// Check if a user has wildcard access to all magic keys
fn user_has_wildcard_access(analyst_access: &HashMap<String, Vec<String>>, username: &str) -> bool {
    if let Some(allowed_keys) = analyst_access.get(username) {
        allowed_keys.iter().any(|k| k == "*")
    } else {
        false
    }
}

/// Get the list of specific magic keys a user can access (empty if wildcard)
fn user_allowed_keys(analyst_access: &HashMap<String, Vec<String>>, username: &str) -> Vec<String> {
    analyst_access.get(username).cloned().unwrap_or_default()
}

// ============================================================================
// List Sessions Endpoint
// ============================================================================

/// Query parameters for listing sessions
#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub magic_key: String,
}

/// Summary information about a survey session
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub magic_key: String,
    pub start_time: i64,
    pub last_update_time: i64,
    pub has_pcap: bool,
    pub has_keylog: bool,
    pub recording_count: i32,
}

/// List sessions by magic key
pub async fn list_sessions(
    State(state): State<Arc<AnalystState>>,
    session_data: Option<Extension<SessionData>>,
    Query(query): Query<ListSessionsQuery>,
) -> Result<Json<Vec<SessionSummary>>, StatusCode> {
    // Check access control
    if let Some(Extension(session)) = &session_data {
        if !user_has_access(&state.analyst_access, &session.handle, &query.magic_key) {
            tracing::warn!(
                "User {} denied access to magic key {}",
                session.handle,
                query.magic_key
            );
            return Err(StatusCode::FORBIDDEN);
        }
    }

    let mut result: Vec<SessionSummary> = {
        let db = state.db.lock().await;

        let mut stmt = db
            .prepare(
                "SELECT s.session_id, s.magic_key, s.start_time, s.last_update_time,
                        s.pcap_path, s.keylog_path,
                        COUNT(r.recording_id) as recording_count
                 FROM survey_sessions s
                 LEFT JOIN recordings r ON s.session_id = r.session_id AND r.deleted = 0
                 WHERE s.magic_key = ? AND s.deleted = 0
                 GROUP BY s.session_id
                 ORDER BY s.start_time DESC",
            )
            .map_err(|e| {
                tracing::error!("Failed to prepare sessions query: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let sessions = stmt
            .query_map(params![&query.magic_key], |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    magic_key: row.get(1)?,
                    start_time: row.get(2)?,
                    last_update_time: row.get(3)?,
                    has_pcap: row.get::<_, Option<String>>(4)?.is_some(),
                    has_keylog: row.get::<_, Option<String>>(5)?.is_some(),
                    recording_count: row.get(6)?,
                })
            })
            .map_err(|e| {
                tracing::error!("Failed to query sessions: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        sessions.collect::<Result<Vec<_>, _>>().map_err(|e| {
            tracing::error!("Failed to collect sessions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    };

    // Check in-memory services for PCAP/keylog availability
    for session in &mut result {
        if !session.has_pcap {
            if let Some(ref capture_service) = state.capture_service {
                session.has_pcap = capture_service.has_session_registered(&session.session_id);
            }
        }
        if !session.has_keylog {
            if let Some(ref keylog_service) = state.keylog_service {
                session.has_keylog =
                    keylog_service.has_keylogs_for_session(&session.session_id);
            }
        }
    }

    Ok(Json(result))
}

// ============================================================================
// Get Session Details Endpoint
// ============================================================================

/// Detailed information about a survey session
#[derive(Debug, Serialize)]
pub struct SessionDetails {
    pub session_id: String,
    pub magic_key: String,
    pub user_login: Option<String>,
    pub start_time: i64,
    pub last_update_time: i64,
    pub pcap_path: Option<String>,
    pub keylog_path: Option<String>,
    pub has_pcap: bool,
    pub has_keylog: bool,
    pub recordings: Vec<RecordingSummary>,
    pub metric_count: i32,
}

/// Summary information about a recording
#[derive(Debug, Serialize)]
pub struct RecordingSummary {
    pub recording_id: String,
    pub video_size_bytes: i64,
    pub sensor_size_bytes: i64,
    pub upload_status: String,
    pub user_notes: Option<String>,
    pub device_info_json: Option<String>,
    pub completed_at: Option<i64>,
}

/// Get session details including recordings
pub async fn get_session(
    State(state): State<Arc<AnalystState>>,
    session_data: Option<Extension<SessionData>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionDetails>, StatusCode> {
    let (session, recordings, metric_count) = {
        let db = state.db.lock().await;

        // Get session info
        let session: (String, Option<String>, i64, i64, Option<String>, Option<String>) = db
            .query_row(
                "SELECT magic_key, user_login, start_time, last_update_time, pcap_path, keylog_path
                 FROM survey_sessions WHERE session_id = ? AND deleted = 0",
                params![&session_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .map_err(|e| {
                tracing::warn!("Session not found: {} - {}", session_id, e);
                StatusCode::NOT_FOUND
            })?;

        // Check access control for the session's magic key
        if let Some(Extension(session_info)) = &session_data {
            if !user_has_access(&state.analyst_access, &session_info.handle, &session.0) {
                tracing::warn!(
                    "User {} denied access to session {} (magic key {})",
                    session_info.handle,
                    session_id,
                    session.0
                );
                return Err(StatusCode::FORBIDDEN);
            }
        }

        // Get recordings
        let mut recordings_stmt = db
            .prepare(
                "SELECT recording_id, video_size_bytes, sensor_size_bytes, upload_status,
                        user_notes, device_info_json, completed_at
                 FROM recordings WHERE session_id = ? AND deleted = 0",
            )
            .map_err(|e| {
                tracing::error!("Failed to prepare recordings query: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let recordings: Vec<RecordingSummary> = recordings_stmt
            .query_map(params![&session_id], |row| {
                Ok(RecordingSummary {
                    recording_id: row.get(0)?,
                    video_size_bytes: row.get(1)?,
                    sensor_size_bytes: row.get(2)?,
                    upload_status: row.get(3)?,
                    user_notes: row.get(4)?,
                    device_info_json: row.get(5)?,
                    completed_at: row.get(6)?,
                })
            })
            .map_err(|e| {
                tracing::error!("Failed to query recordings: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failed to collect recordings: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // Get metric count
        let metric_count: i32 = db
            .query_row(
                "SELECT COUNT(*) FROM survey_metrics WHERE session_id = ? AND deleted = 0",
                params![&session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        (session, recordings, metric_count)
    };

    // Check data availability from DB paths and in-memory services
    let mut has_pcap = session.4.is_some();
    let mut has_keylog = session.5.is_some();

    if !has_pcap {
        if let Some(ref capture_service) = state.capture_service {
            has_pcap = capture_service.has_session_registered(&session_id);
        }
    }
    if !has_keylog {
        if let Some(ref keylog_service) = state.keylog_service {
            has_keylog = keylog_service.has_keylogs_for_session(&session_id);
        }
    }

    Ok(Json(SessionDetails {
        session_id: session_id.clone(),
        magic_key: session.0,
        user_login: session.1,
        start_time: session.2,
        last_update_time: session.3,
        pcap_path: session.4,
        keylog_path: session.5,
        has_pcap,
        has_keylog,
        recordings,
        metric_count,
    }))
}

// ============================================================================
// List All Magic Keys Endpoint
// ============================================================================

/// Summary of sessions for a magic key
#[derive(Debug, Serialize)]
pub struct MagicKeySummary {
    pub magic_key: String,
    pub session_count: i32,
    pub total_recordings: i32,
    pub latest_session_time: Option<i64>,
}

/// List all magic keys with session counts (filtered by user access)
pub async fn list_magic_keys(
    State(state): State<Arc<AnalystState>>,
    session_data: Option<Extension<SessionData>>,
) -> Result<Json<Vec<MagicKeySummary>>, StatusCode> {
    let db = state.db.lock().await;

    let mut stmt = db
        .prepare(
            "SELECT s.magic_key,
                    COUNT(DISTINCT s.session_id) as session_count,
                    COUNT(r.recording_id) as total_recordings,
                    MAX(s.start_time) as latest_session_time
             FROM survey_sessions s
             LEFT JOIN recordings r ON s.session_id = r.session_id AND r.deleted = 0
             WHERE s.deleted = 0
             GROUP BY s.magic_key
             ORDER BY latest_session_time DESC",
        )
        .map_err(|e| {
            tracing::error!("Failed to prepare magic keys query: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let keys = stmt
        .query_map([], |row| {
            Ok(MagicKeySummary {
                magic_key: row.get(0)?,
                session_count: row.get(1)?,
                total_recordings: row.get(2)?,
                latest_session_time: row.get(3)?,
            })
        })
        .map_err(|e| {
            tracing::error!("Failed to query magic keys: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let all_keys: Vec<MagicKeySummary> = keys
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            tracing::error!("Failed to collect magic keys: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Filter by user access if session data is available
    let filtered = if let Some(Extension(session)) = &session_data {
        if user_has_wildcard_access(&state.analyst_access, &session.handle) {
            all_keys
        } else {
            let allowed = user_allowed_keys(&state.analyst_access, &session.handle);
            all_keys
                .into_iter()
                .filter(|k| allowed.contains(&k.magic_key))
                .collect()
        }
    } else {
        all_keys
    };

    Ok(Json(filtered))
}

// ============================================================================
// Get User's Allowed Magic Keys Endpoint
// ============================================================================

/// Response for the allowed keys endpoint
#[derive(Debug, Serialize)]
pub struct AllowedKeysResponse {
    pub username: String,
    pub has_wildcard: bool,
    pub allowed_keys: Vec<String>,
}

/// Get the current user's allowed magic keys from configuration
pub async fn get_allowed_keys(
    State(state): State<Arc<AnalystState>>,
    session_data: Option<Extension<SessionData>>,
) -> Result<Json<AllowedKeysResponse>, StatusCode> {
    let username = session_data
        .as_ref()
        .map(|Extension(s)| s.handle.clone())
        .unwrap_or_default();

    let has_wildcard = user_has_wildcard_access(&state.analyst_access, &username);
    let allowed_keys = user_allowed_keys(&state.analyst_access, &username);

    Ok(Json(AllowedKeysResponse {
        username,
        has_wildcard,
        allowed_keys,
    }))
}

// ============================================================================
// Recording File Download Endpoints
// ============================================================================

use axum::{
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
};

/// Serve a recording's video file for inline playback (supports HTTP Range requests)
pub async fn download_recording_video(
    State(state): State<Arc<AnalystState>>,
    session_data: Option<Extension<SessionData>>,
    headers: HeaderMap,
    Path(recording_id): Path<String>,
) -> Result<Response, StatusCode> {
    serve_recording_file(&state, &session_data, &recording_id, "video", Some(&headers)).await
}

/// Serve a recording's sensor data file for download
pub async fn download_recording_sensor(
    State(state): State<Arc<AnalystState>>,
    session_data: Option<Extension<SessionData>>,
    Path(recording_id): Path<String>,
) -> Result<Response, StatusCode> {
    serve_recording_file(&state, &session_data, &recording_id, "sensor", None).await
}

/// Internal helper to serve a recording file (video or sensor) with access control
/// Supports HTTP Range requests for video files to enable proper browser playback.
async fn serve_recording_file(
    state: &AnalystState,
    session_data: &Option<Extension<SessionData>>,
    recording_id: &str,
    file_type: &str,
    headers: Option<&HeaderMap>,
) -> Result<Response, StatusCode> {
    let db = state.db.lock().await;

    // Get recording details and the associated session's magic key
    let query = match file_type {
        "video" => "SELECT r.video_path, s.magic_key FROM recordings r
                    JOIN survey_sessions s ON r.session_id = s.session_id
                    WHERE r.recording_id = ? AND r.deleted = 0 AND s.deleted = 0",
        "sensor" => "SELECT r.sensor_path, s.magic_key FROM recordings r
                     JOIN survey_sessions s ON r.session_id = s.session_id
                     WHERE r.recording_id = ? AND r.deleted = 0 AND s.deleted = 0",
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let (file_path, magic_key): (String, String) = db
        .query_row(query, params![recording_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|_| {
            tracing::warn!("Recording not found: {}", recording_id);
            StatusCode::NOT_FOUND
        })?;

    // Check access control
    if let Some(Extension(session_info)) = session_data {
        if !user_has_access(&state.analyst_access, &session_info.handle, &magic_key) {
            tracing::warn!(
                "User {} denied access to recording {} (magic key {})",
                session_info.handle,
                recording_id,
                magic_key
            );
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // Drop DB lock before file I/O
    drop(db);

    // Read the file
    let data = tokio::fs::read(&file_path).await.map_err(|e| {
        tracing::error!("Failed to read file {}: {}", file_path, e);
        StatusCode::NOT_FOUND
    })?;

    let total_size = data.len();

    let (content_type, filename) = match file_type {
        "video" => ("video/webm", format!("{}.webm", recording_id)),
        "sensor" => (
            "application/json",
            format!("{}_sensors.json", recording_id),
        ),
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let disposition = if file_type == "video" {
        format!("inline; filename=\"{}\"", filename)
    } else {
        format!("attachment; filename=\"{}\"", filename)
    };

    // Handle HTTP Range requests for video files
    if let Some(hdrs) = headers {
        if let Some(range_header) = hdrs.get(header::RANGE) {
            if let Ok(range_str) = range_header.to_str() {
                if let Some((start, end)) = parse_byte_range(range_str, total_size) {
                    let content_length = end - start + 1;
                    let body_data = data[start..=end].to_vec();

                    return Ok(Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(header::CONTENT_TYPE, content_type)
                        .header(header::CONTENT_LENGTH, content_length.to_string())
                        .header(header::CONTENT_DISPOSITION, &disposition)
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(
                            header::CONTENT_RANGE,
                            format!("bytes {}-{}/{}", start, end, total_size),
                        )
                        .body(axum::body::Body::from(body_data))
                        .unwrap()
                        .into_response());
                }
            }
        }
    }

    // Full file response (no Range header or non-video)
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, total_size.to_string())
        .header(header::CONTENT_DISPOSITION, &disposition)
        .header(header::ACCEPT_RANGES, "bytes")
        .body(axum::body::Body::from(data))
        .unwrap()
        .into_response())
}

/// Parse an HTTP Range header value like "bytes=0-1023" or "bytes=1024-" or "bytes=-500"
/// Returns Some((start, end)) inclusive byte range, or None if unparseable.
fn parse_byte_range(range_str: &str, total_size: usize) -> Option<(usize, usize)> {
    let range_str = range_str.trim();
    if !range_str.starts_with("bytes=") {
        return None;
    }
    let range_spec = &range_str["bytes=".len()..];
    // Only handle the first range in a multi-range request
    let range_spec = range_spec.split(',').next()?.trim();

    if let Some(suffix) = range_spec.strip_prefix('-') {
        // Suffix range: last N bytes
        let suffix_len: usize = suffix.parse().ok()?;
        if suffix_len == 0 || suffix_len > total_size {
            return None;
        }
        Some((total_size - suffix_len, total_size - 1))
    } else if let Some((start_str, end_str)) = range_spec.split_once('-') {
        let start: usize = start_str.parse().ok()?;
        if start >= total_size {
            return None;
        }
        let end = if end_str.is_empty() {
            total_size - 1
        } else {
            let end: usize = end_str.parse().ok()?;
            end.min(total_size - 1)
        };
        if start > end {
            return None;
        }
        Some((start, end))
    } else {
        None
    }
}

// ============================================================================
// Session Metrics Endpoint
// ============================================================================

/// A single metric data point for charting
#[derive(Debug, Serialize)]
pub struct MetricEntry {
    pub timestamp_ms: i64,
    pub source: String,
    pub conn_id: Option<String>,
    pub direction: Option<String>,
    pub delay_p50_ms: Option<f64>,
    pub delay_p99_ms: Option<f64>,
    pub delay_min_ms: Option<f64>,
    pub delay_max_ms: Option<f64>,
    pub jitter_p50_ms: Option<f64>,
    pub jitter_p99_ms: Option<f64>,
    pub rtt_p50_ms: Option<f64>,
    pub rtt_p99_ms: Option<f64>,
    pub loss_rate: Option<f64>,
    pub reorder_rate: Option<f64>,
    pub probe_count: Option<i32>,
    pub baseline_delay_ms: Option<f64>,
}

/// Get metrics for a session (for charting latency, jitter, loss over time)
pub async fn get_session_metrics(
    State(state): State<Arc<AnalystState>>,
    session_data: Option<Extension<SessionData>>,
    Path(session_id): Path<String>,
) -> Result<Json<Vec<MetricEntry>>, StatusCode> {
    let db = state.db.lock().await;

    // First check the session exists and get the magic key for access control
    let magic_key: String = db
        .query_row(
            "SELECT magic_key FROM survey_sessions WHERE session_id = ? AND deleted = 0",
            params![&session_id],
            |row| row.get(0),
        )
        .map_err(|_| {
            tracing::warn!("Session not found for metrics: {}", session_id);
            StatusCode::NOT_FOUND
        })?;

    // Check access control
    if let Some(Extension(session_info)) = &session_data {
        if !user_has_access(&state.analyst_access, &session_info.handle, &magic_key) {
            tracing::warn!(
                "User {} denied access to metrics for session {} (magic key {})",
                session_info.handle,
                session_id,
                magic_key
            );
            return Err(StatusCode::FORBIDDEN);
        }
    }

    let mut stmt = db
        .prepare(
            "SELECT timestamp_ms, source, conn_id, direction,
                    delay_p50_ms, delay_p99_ms, delay_min_ms, delay_max_ms,
                    jitter_p50_ms, jitter_p99_ms,
                    rtt_p50_ms, rtt_p99_ms,
                    loss_rate, reorder_rate, probe_count, baseline_delay_ms
             FROM survey_metrics
             WHERE session_id = ? AND deleted = 0
             ORDER BY timestamp_ms ASC",
        )
        .map_err(|e| {
            tracing::error!("Failed to prepare metrics query: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let metrics = stmt
        .query_map(params![&session_id], |row| {
            Ok(MetricEntry {
                timestamp_ms: row.get(0)?,
                source: row.get(1)?,
                conn_id: row.get(2)?,
                direction: row.get(3)?,
                delay_p50_ms: row.get(4)?,
                delay_p99_ms: row.get(5)?,
                delay_min_ms: row.get(6)?,
                delay_max_ms: row.get(7)?,
                jitter_p50_ms: row.get(8)?,
                jitter_p99_ms: row.get(9)?,
                rtt_p50_ms: row.get(10)?,
                rtt_p99_ms: row.get(11)?,
                loss_rate: row.get(12)?,
                reorder_rate: row.get(13)?,
                probe_count: row.get(14)?,
                baseline_delay_ms: row.get(15)?,
            })
        })
        .map_err(|e| {
            tracing::error!("Failed to query metrics: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let result: Vec<MetricEntry> = metrics
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            tracing::error!("Failed to collect metrics: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(result))
}
