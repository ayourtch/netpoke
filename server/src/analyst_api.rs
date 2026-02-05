//! Analyst API for browsing survey data
//!
//! Provides endpoints for analysts to list and view survey sessions, recordings,
//! and metrics for analysis and export.

use crate::database::DbConnection;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// State shared by analyst API handlers
#[derive(Clone)]
pub struct AnalystState {
    pub db: DbConnection,
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
    Query(query): Query<ListSessionsQuery>,
) -> Result<Json<Vec<SessionSummary>>, StatusCode> {
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

    let result: Result<Vec<_>, _> = sessions.collect();
    Ok(Json(result.map_err(|e| {
        tracing::error!("Failed to collect sessions: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?))
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
    Path(session_id): Path<String>,
) -> Result<Json<SessionDetails>, StatusCode> {
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

    Ok(Json(SessionDetails {
        session_id: session_id.clone(),
        magic_key: session.0,
        user_login: session.1,
        start_time: session.2,
        last_update_time: session.3,
        pcap_path: session.4,
        keylog_path: session.5,
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

/// List all magic keys with session counts
pub async fn list_magic_keys(
    State(state): State<Arc<AnalystState>>,
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

    let result: Result<Vec<_>, _> = keys.collect();
    Ok(Json(result.map_err(|e| {
        tracing::error!("Failed to collect magic keys: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?))
}
