//! Upload API for survey recordings
//!
//! Provides endpoints for chunked, resumable uploads of video and sensor data.
//! The upload protocol consists of three steps:
//! 1. `prepare` - Initialize upload and get existing chunk info for resume
//! 2. `chunk` - Upload individual chunks with checksum verification
//! 3. `finalize` - Verify complete file integrity and mark as complete

use crate::database::DbConnection;
use crate::upload_utils::{calculate_checksum, calculate_combined_checksum, calculate_file_checksums, CHUNK_SIZE};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

/// State shared by upload API handlers
#[derive(Clone)]
pub struct UploadState {
    pub db: Option<DbConnection>,
    pub storage_base_path: String,
}

/// Error response for upload API
///
/// This struct is serialized to JSON and sent to clients, so the `error` field
/// must be public for serde serialization. The helper methods provide standard
/// error messages for common error conditions.
#[derive(Debug, Serialize)]
pub struct UploadError {
    /// Human-readable error message describing what went wrong
    pub error: String,
}

impl UploadError {
    fn database_unavailable() -> Self {
        Self {
            error: "Upload feature is unavailable - database not configured".to_string(),
        }
    }

    fn session_not_found(session_id: &str) -> Self {
        Self {
            error: format!("Session not found: {}", session_id),
        }
    }

    fn recording_not_found(recording_id: &str) -> Self {
        Self {
            error: format!("Recording not found: {}", recording_id),
        }
    }
}

// ============================================================================
// Prepare Upload Endpoint
// ============================================================================

/// Request body for prepare upload endpoint
#[derive(Debug, Deserialize)]
pub struct PrepareUploadRequest {
    pub session_id: String,
    pub recording_id: String,
    pub video_size_bytes: u64,
    pub sensor_size_bytes: u64,
    pub device_info: serde_json::Value,
    pub user_notes: Option<String>,
}

/// Information about an existing chunk (for resume)
#[derive(Debug, Serialize)]
pub struct ChunkInfo {
    pub chunk_index: usize,
    pub checksum: String,
}

/// Response from prepare upload endpoint
#[derive(Debug, Serialize)]
pub struct PrepareUploadResponse {
    pub recording_id: String,
    pub video_chunks: Vec<Option<ChunkInfo>>,
    pub sensor_chunks: Vec<Option<ChunkInfo>>,
    pub video_uploaded_bytes: u64,
    pub sensor_uploaded_bytes: u64,
}

/// Prepare upload endpoint - validates session and returns existing chunks for resume
pub async fn prepare_upload(
    State(state): State<Arc<UploadState>>,
    Json(req): Json<PrepareUploadRequest>,
) -> Result<Json<PrepareUploadResponse>, (StatusCode, Json<UploadError>)> {
    // Step 0: Check if database is available
    let db = state.db.as_ref().ok_or_else(|| {
        tracing::warn!("Upload prepare failed: database not configured");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(UploadError::database_unavailable()),
        )
    })?;

    // Step 1: Verify session exists
    let session_exists = {
        let db = db.lock().await;
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM survey_sessions WHERE session_id = ? AND deleted = 0",
                params![&req.session_id],
                |row| row.get(0),
            )
            .map_err(|e| {
                tracing::error!("Database error checking session: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(UploadError {
                        error: "Database error".to_string(),
                    }),
                )
            })?;
        count > 0
    };

    if !session_exists {
        tracing::warn!("Upload prepare: session not found: {}", req.session_id);
        return Err((
            StatusCode::NOT_FOUND,
            Json(UploadError::session_not_found(&req.session_id)),
        ));
    }

    // Step 2: Get session details for path construction
    let (magic_key, start_time): (String, i64) = {
        let db = db.lock().await;
        db.query_row(
            "SELECT magic_key, start_time FROM survey_sessions WHERE session_id = ?",
            params![&req.session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| {
            tracing::error!("Database error getting session details: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Database error".to_string(),
                }),
            )
        })?
    };

    // Step 3: Construct file paths
    let start_dt = chrono::DateTime::from_timestamp_millis(start_time)
        .ok_or_else(|| {
            tracing::error!("Invalid timestamp: {}", start_time);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Invalid session timestamp".to_string(),
                }),
            )
        })?;

    let session_dir = PathBuf::from(&state.storage_base_path)
        .join(&magic_key)
        .join(start_dt.format("%Y").to_string())
        .join(start_dt.format("%m").to_string())
        .join(start_dt.format("%d").to_string())
        .join(&req.session_id);

    // Create directory if it doesn't exist
    tokio::fs::create_dir_all(&session_dir)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create session directory: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to create storage directory".to_string(),
                }),
            )
        })?;

    let video_path = session_dir.join(format!("{}.webm", req.recording_id));
    let sensor_path = session_dir.join(format!("{}.json", req.recording_id));

    // Step 4: Calculate existing checksums if files exist (for resume)
    let video_chunks = calculate_file_checksums(&video_path, req.video_size_bytes)
        .await
        .map_err(|e| {
            tracing::error!("Failed to calculate video checksums: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to process video file".to_string(),
                }),
            )
        })?
        .into_iter()
        .enumerate()
        .map(|(idx, cs)| {
            cs.map(|checksum| ChunkInfo {
                chunk_index: idx,
                checksum,
            })
        })
        .collect();

    let sensor_chunks = calculate_file_checksums(&sensor_path, req.sensor_size_bytes)
        .await
        .map_err(|e| {
            tracing::error!("Failed to calculate sensor checksums: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to process sensor file".to_string(),
                }),
            )
        })?
        .into_iter()
        .enumerate()
        .map(|(idx, cs)| {
            cs.map(|checksum| ChunkInfo {
                chunk_index: idx,
                checksum,
            })
        })
        .collect();

    // Get uploaded bytes from existing files
    let video_uploaded_bytes = if video_path.exists() {
        tokio::fs::metadata(&video_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    let sensor_uploaded_bytes = if sensor_path.exists() {
        tokio::fs::metadata(&sensor_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    // Step 5: Create or update recording in database
    {
        let db = db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        let device_info_json = serde_json::to_string(&req.device_info).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(UploadError {
                    error: "Invalid device info".to_string(),
                }),
            )
        })?;

        db.execute(
            "INSERT INTO recordings (
                recording_id, session_id, video_path, sensor_path,
                video_size_bytes, sensor_size_bytes,
                video_uploaded_bytes, sensor_uploaded_bytes,
                device_info_json, user_notes, created_at, upload_status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(recording_id) DO UPDATE SET
                video_uploaded_bytes = ?,
                sensor_uploaded_bytes = ?,
                user_notes = COALESCE(?, user_notes)",
            params![
                &req.recording_id,
                &req.session_id,
                video_path.to_string_lossy().as_ref(),
                sensor_path.to_string_lossy().as_ref(),
                req.video_size_bytes,
                req.sensor_size_bytes,
                video_uploaded_bytes,
                sensor_uploaded_bytes,
                device_info_json,
                req.user_notes,
                now_ms,
                "uploading",
                video_uploaded_bytes,
                sensor_uploaded_bytes,
                req.user_notes
            ],
        )
        .map_err(|e| {
            tracing::error!("Database error creating recording: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to create recording".to_string(),
                }),
            )
        })?;
    }

    tracing::info!(
        "Prepared upload for recording {} in session {}",
        req.recording_id,
        req.session_id
    );

    Ok(Json(PrepareUploadResponse {
        recording_id: req.recording_id,
        video_chunks,
        sensor_chunks,
        video_uploaded_bytes,
        sensor_uploaded_bytes,
    }))
}

// ============================================================================
// Upload Chunk Endpoint
// ============================================================================

/// Response from chunk upload endpoint
#[derive(Debug, Serialize)]
pub struct ChunkUploadResponse {
    pub status: String,
    pub chunk_index: usize,
    pub bytes_received: usize,
}

/// Upload chunk endpoint - receives individual chunks with checksum verification
///
/// Headers required:
/// - X-Recording-Id: UUID of the recording
/// - X-File-Type: "video" or "sensor"
/// - X-Chunk-Index: 0-based index of the chunk
/// - X-Chunk-Checksum: SHA-256 hex string of the chunk data
pub async fn upload_chunk(
    State(state): State<Arc<UploadState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ChunkUploadResponse>, (StatusCode, Json<UploadError>)> {
    // Step 0: Check if database is available
    let db = state.db.as_ref().ok_or_else(|| {
        tracing::warn!("Upload chunk failed: database not configured");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(UploadError::database_unavailable()),
        )
    })?;

    // Step 1: Extract headers
    let recording_id = headers
        .get("X-Recording-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing X-Recording-Id header");
            (
                StatusCode::BAD_REQUEST,
                Json(UploadError {
                    error: "Missing X-Recording-Id header".to_string(),
                }),
            )
        })?;

    let file_type = headers
        .get("X-File-Type")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing X-File-Type header");
            (
                StatusCode::BAD_REQUEST,
                Json(UploadError {
                    error: "Missing X-File-Type header".to_string(),
                }),
            )
        })?;

    if file_type != "video" && file_type != "sensor" {
        tracing::warn!("Invalid file type: {}", file_type);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(UploadError {
                error: format!("Invalid file type: {}", file_type),
            }),
        ));
    }

    let chunk_index: usize = headers
        .get("X-Chunk-Index")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing or invalid X-Chunk-Index header");
            (
                StatusCode::BAD_REQUEST,
                Json(UploadError {
                    error: "Missing or invalid X-Chunk-Index header".to_string(),
                }),
            )
        })?;

    let expected_checksum = headers
        .get("X-Chunk-Checksum")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing X-Chunk-Checksum header");
            (
                StatusCode::BAD_REQUEST,
                Json(UploadError {
                    error: "Missing X-Chunk-Checksum header".to_string(),
                }),
            )
        })?;

    // Step 2: Verify checksum
    let actual_checksum = calculate_checksum(&body);
    if actual_checksum != expected_checksum {
        tracing::warn!(
            "Checksum mismatch for recording {} chunk {}: expected {}, got {}",
            recording_id,
            chunk_index,
            expected_checksum,
            actual_checksum
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(UploadError {
                error: "Checksum mismatch".to_string(),
            }),
        ));
    }

    // Step 3: Get file path from database
    let file_path = {
        let db = db.lock().await;
        
        // Use explicit queries to avoid any SQL injection risk
        let path: String = match file_type {
            "video" => db.query_row(
                "SELECT video_path FROM recordings WHERE recording_id = ?",
                params![recording_id],
                |row| row.get(0),
            ),
            "sensor" => db.query_row(
                "SELECT sensor_path FROM recordings WHERE recording_id = ?",
                params![recording_id],
                |row| row.get(0),
            ),
            _ => {
                tracing::warn!("Invalid file type: {}", file_type);
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(UploadError {
                        error: format!("Invalid file type: {}", file_type),
                    }),
                ));
            }
        }
        .map_err(|_| {
            tracing::warn!("Recording not found: {}", recording_id);
            (
                StatusCode::NOT_FOUND,
                Json(UploadError::recording_not_found(recording_id)),
            )
        })?;
        PathBuf::from(path)
    };

    // Step 4: Open file and write chunk at offset
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&file_path)
        .await
        .map_err(|e| {
            tracing::error!("Failed to open file for writing: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to open file for writing".to_string(),
                }),
            )
        })?;

    let offset = (chunk_index * CHUNK_SIZE) as u64;
    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|e| {
            tracing::error!("Failed to seek in file: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to seek in file".to_string(),
                }),
            )
        })?;

    file.write_all(&body).await.map_err(|e| {
        tracing::error!("Failed to write chunk: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(UploadError {
                error: "Failed to write chunk".to_string(),
            }),
        )
    })?;

    file.flush().await.map_err(|e| {
        tracing::error!("Failed to flush file: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(UploadError {
                error: "Failed to flush file".to_string(),
            }),
        )
    })?;

    // Step 5: Update uploaded bytes in database
    let bytes_received = body.len();
    {
        let db = db.lock().await;
        let new_uploaded = offset + bytes_received as u64;
        
        // Use explicit queries to avoid any SQL injection risk
        match file_type {
            "video" => db.execute(
                "UPDATE recordings SET video_uploaded_bytes = MAX(video_uploaded_bytes, ?) WHERE recording_id = ?",
                params![new_uploaded, recording_id],
            ),
            "sensor" => db.execute(
                "UPDATE recordings SET sensor_uploaded_bytes = MAX(sensor_uploaded_bytes, ?) WHERE recording_id = ?",
                params![new_uploaded, recording_id],
            ),
            _ => {
                tracing::warn!("Invalid file type: {}", file_type);
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(UploadError {
                        error: format!("Invalid file type: {}", file_type),
                    }),
                ));
            }
        }
        .map_err(|e| {
            tracing::error!("Failed to update uploaded bytes: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to update uploaded bytes".to_string(),
                }),
            )
        })?;
    }

    tracing::debug!(
        "Received chunk {} for {} file of recording {}",
        chunk_index,
        file_type,
        recording_id
    );

    Ok(Json(ChunkUploadResponse {
        status: "received".to_string(),
        chunk_index,
        bytes_received,
    }))
}

// ============================================================================
// Finalize Upload Endpoint
// ============================================================================

/// Request body for finalize upload endpoint
#[derive(Debug, Deserialize)]
pub struct FinalizeUploadRequest {
    pub recording_id: String,
    pub video_final_checksum: String,
    pub sensor_final_checksum: String,
}

/// Response from finalize upload endpoint
#[derive(Debug, Serialize)]
pub struct FinalizeUploadResponse {
    pub status: String,
    pub video_verified: bool,
    pub sensor_verified: bool,
}

/// Finalize upload endpoint - verifies complete file integrity
pub async fn finalize_upload(
    State(state): State<Arc<UploadState>>,
    Json(req): Json<FinalizeUploadRequest>,
) -> Result<Json<FinalizeUploadResponse>, (StatusCode, Json<UploadError>)> {
    // Step 0: Check if database is available
    let db = state.db.as_ref().ok_or_else(|| {
        tracing::warn!("Upload finalize failed: database not configured");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(UploadError::database_unavailable()),
        )
    })?;

    // Step 1: Get recording details
    let (video_path, sensor_path, video_size, sensor_size): (String, String, i64, i64) = {
        let db = db.lock().await;
        db.query_row(
            "SELECT video_path, sensor_path, video_size_bytes, sensor_size_bytes
             FROM recordings WHERE recording_id = ?",
            params![&req.recording_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| {
            tracing::warn!("Recording not found for finalize: {}", req.recording_id);
            (
                StatusCode::NOT_FOUND,
                Json(UploadError::recording_not_found(&req.recording_id)),
            )
        })?
    };

    // Step 2: Calculate checksums for video
    let video_checksums = calculate_file_checksums(&PathBuf::from(&video_path), video_size as u64)
        .await
        .map_err(|e| {
            tracing::error!("Failed to calculate video checksums: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to verify video file".to_string(),
                }),
            )
        })?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    let video_combined = calculate_combined_checksum(&video_checksums);
    let video_verified = video_combined == req.video_final_checksum;

    // Step 3: Calculate checksums for sensor
    let sensor_checksums =
        calculate_file_checksums(&PathBuf::from(&sensor_path), sensor_size as u64)
            .await
            .map_err(|e| {
                tracing::error!("Failed to calculate sensor checksums: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(UploadError {
                        error: "Failed to verify sensor file".to_string(),
                    }),
                )
            })?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

    let sensor_combined = calculate_combined_checksum(&sensor_checksums);
    let sensor_verified = sensor_combined == req.sensor_final_checksum;

    // Step 4: Update recording status
    if video_verified && sensor_verified {
        let db = db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        db.execute(
            "UPDATE recordings SET upload_status = 'complete', completed_at = ? WHERE recording_id = ?",
            params![now_ms, &req.recording_id],
        )
        .map_err(|e| {
            tracing::error!("Failed to update recording status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to update recording status".to_string(),
                }),
            )
        })?;

        tracing::info!("Upload finalized successfully: {}", req.recording_id);
    } else {
        let db = db.lock().await;
        db.execute(
            "UPDATE recordings SET upload_status = 'failed' WHERE recording_id = ?",
            params![&req.recording_id],
        )
        .map_err(|e| {
            tracing::error!("Failed to update recording status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadError {
                    error: "Failed to update recording status".to_string(),
                }),
            )
        })?;

        tracing::warn!(
            "Upload verification failed for {}: video={}, sensor={}",
            req.recording_id,
            video_verified,
            sensor_verified
        );
    }

    Ok(Json(FinalizeUploadResponse {
        status: if video_verified && sensor_verified {
            "complete".to_string()
        } else {
            "failed".to_string()
        },
        video_verified,
        sensor_verified,
    }))
}
