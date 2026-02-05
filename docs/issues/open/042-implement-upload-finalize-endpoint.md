# Issue 042: Implement Upload Finalize Endpoint

## Summary
Create the `/api/upload/finalize` endpoint that verifies complete file integrity and marks uploads as complete.

## Location
- File: `server/src/upload_api.rs` (add to existing file)

## Current Behavior
Prepare and chunk upload endpoints exist; finalization is not implemented.

## Expected Behavior
An endpoint that:
1. Calculates checksums for all chunks of uploaded files
2. Verifies combined checksum matches client-provided value
3. Updates recording status to "complete" or "failed"
4. Returns verification status for both video and sensor files

## Impact
Final step in the upload API, ensuring complete and verified uploads.

## Suggested Implementation

### Step 1: Add finalize handler to upload_api.rs

Add to `server/src/upload_api.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct FinalizeUploadRequest {
    pub recording_id: String,
    pub video_final_checksum: String,
    pub sensor_final_checksum: String,
}

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
) -> Result<Json<FinalizeUploadResponse>, StatusCode> {
    // Step 1: Get recording details
    let (video_path, sensor_path, video_size, sensor_size): (String, String, i64, i64) = {
        let db = state.db.lock().await;
        db.query_row(
            "SELECT video_path, sensor_path, video_size_bytes, sensor_size_bytes
             FROM recordings WHERE recording_id = ?",
            params![&req.recording_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| StatusCode::NOT_FOUND)?
    };

    // Step 2: Calculate checksums for video
    let video_checksums = calculate_file_checksums(
        &PathBuf::from(&video_path),
        video_size as u64,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let video_combined = crate::upload_utils::calculate_combined_checksum(&video_checksums);
    let video_verified = video_combined == req.video_final_checksum;

    // Step 3: Calculate checksums for sensor
    let sensor_checksums = calculate_file_checksums(
        &PathBuf::from(&sensor_path),
        sensor_size as u64,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let sensor_combined = crate::upload_utils::calculate_combined_checksum(&sensor_checksums);
    let sensor_verified = sensor_combined == req.sensor_final_checksum;

    // Step 4: Update recording status
    if video_verified && sensor_verified {
        let db = state.db.lock().await;
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;

        db.execute(
            "UPDATE recordings SET upload_status = 'complete', completed_at = ? WHERE recording_id = ?",
            params![now_ms, &req.recording_id],
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        let db = state.db.lock().await;
        db.execute(
            "UPDATE recordings SET upload_status = 'failed' WHERE recording_id = ?",
            params![&req.recording_id],
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
```

## Request Format
```json
{
  "recording_id": "d4e5f6a7-...",
  "video_final_checksum": "sha256:hash_of_all_video_chunk_hashes",
  "sensor_final_checksum": "sha256:hash_of_all_sensor_chunk_hashes"
}
```

## Response Format
```json
{
  "status": "complete",
  "video_verified": true,
  "sensor_verified": true
}
```

## Verification Logic
1. Read all chunks from each file
2. Calculate SHA-256 for each chunk
3. Concatenate chunk hashes and hash the result
4. Compare with client-provided final checksum

## Testing
- Build succeeds: `cargo build`
- Manual test: Complete upload flow and verify finalization

## Dependencies
- Issue 040: Implement upload prepare endpoint
- Issue 041: Implement upload chunk endpoint
- Issue 039: Implement chunk checksum utilities

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 10 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Upload Protocol Step 3.

---
*Created: 2026-02-05*
