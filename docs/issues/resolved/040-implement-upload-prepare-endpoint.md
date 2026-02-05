# Issue 040: Implement Upload Prepare Endpoint

## Summary
Create the `/api/upload/prepare` endpoint that initiates or resumes an upload, returning existing chunk checksums for resume capability.

## Location
- File: `server/src/upload_api.rs` (new file)
- File: `server/src/main.rs` (add mod declaration)

## Current Behavior
No upload API exists.

## Expected Behavior
An endpoint that:
1. Validates the session exists in the database
2. Creates the storage directory structure based on magic key and date
3. Creates or updates the recording record in the database
4. Returns checksums of any existing chunks for resume capability

## Impact
First step in the three-part upload API, enabling upload initiation and resumption.

## Suggested Implementation

### Step 1: Create upload API module

Create `server/src/upload_api.rs` with the prepare endpoint:

```rust
use crate::database::DbConnection;
use crate::upload_utils::{calculate_file_checksums, CHUNK_SIZE};
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct UploadState {
    pub db: DbConnection,
    pub storage_base_path: String,
}

#[derive(Debug, Deserialize)]
pub struct PrepareUploadRequest {
    pub session_id: String,
    pub recording_id: String,
    pub video_size_bytes: u64,
    pub sensor_size_bytes: u64,
    pub device_info: serde_json::Value,
    pub user_notes: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChunkInfo {
    pub chunk_index: usize,
    pub checksum: String,
}

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
) -> Result<Json<PrepareUploadResponse>, StatusCode> {
    // Step 1: Verify session exists
    let session_exists = {
        let db = state.db.lock().await;
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM survey_sessions WHERE session_id = ? AND deleted = 0",
                params![&req.session_id],
                |row| row.get(0),
            )
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        count > 0
    };

    if !session_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    // Step 2: Get session details for path construction
    let (magic_key, start_time): (String, i64) = {
        let db = state.db.lock().await;
        db.query_row(
            "SELECT magic_key, start_time FROM survey_sessions WHERE session_id = ?",
            params![&req.session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    // Step 3: Construct file paths
    let start_dt = chrono::DateTime::from_timestamp_millis(start_time)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let session_dir = PathBuf::from(&state.storage_base_path)
        .join(&magic_key)
        .join(start_dt.format("%Y").to_string())
        .join(start_dt.format("%m").to_string())
        .join(start_dt.format("%d").to_string())
        .join(&req.session_id);

    // Create directory if it doesn't exist
    tokio::fs::create_dir_all(&session_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let video_path = session_dir.join(format!("{}.webm", req.recording_id));
    let sensor_path = session_dir.join(format!("{}.json", req.recording_id));

    // Step 4: Calculate existing checksums if files exist (for resume)
    let video_chunks = calculate_file_checksums(&video_path, req.video_size_bytes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .enumerate()
        .map(|(idx, cs)| cs.map(|checksum| ChunkInfo { chunk_index: idx, checksum }))
        .collect();

    let sensor_chunks = calculate_file_checksums(&sensor_path, req.sensor_size_bytes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .enumerate()
        .map(|(idx, cs)| cs.map(|checksum| ChunkInfo { chunk_index: idx, checksum }))
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
        let db = state.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        let device_info_json = serde_json::to_string(&req.device_info)
            .map_err(|_| StatusCode::BAD_REQUEST)?;

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
                &req.recording_id, &req.session_id,
                video_path.to_string_lossy().as_ref(),
                sensor_path.to_string_lossy().as_ref(),
                req.video_size_bytes, req.sensor_size_bytes,
                video_uploaded_bytes, sensor_uploaded_bytes,
                device_info_json, req.user_notes, now_ms, "uploading",
                video_uploaded_bytes, sensor_uploaded_bytes, req.user_notes
            ],
        ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(PrepareUploadResponse {
        recording_id: req.recording_id,
        video_chunks,
        sensor_chunks,
        video_uploaded_bytes,
        sensor_uploaded_bytes,
    }))
}
```

### Step 2: Add module declaration

Add to `server/src/main.rs`:
```rust
mod upload_api;
```

## Testing
- Build succeeds: `cargo build`
- Manual test: POST to `/api/upload/prepare` with valid session ID

## Dependencies
- Issue 035: Implement database module
- Issue 038: Create SessionManager service
- Issue 039: Implement chunk checksum utilities

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 8 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Upload Protocol section.

---
*Created: 2026-02-05*
---
*Resolved: 2026-02-05*

## Resolution

Implemented as part of the survey upload feature implementation.
