# Issue 041: Implement Upload Chunk Endpoint

## Summary
Create the `/api/upload/chunk` endpoint that receives individual file chunks with checksum verification.

## Location
- File: `server/src/upload_api.rs` (add to existing file)

## Current Behavior
Only the prepare endpoint exists; chunk upload is not implemented.

## Expected Behavior
An endpoint that:
1. Extracts chunk metadata from HTTP headers
2. Verifies the SHA-256 checksum of received data
3. Writes the chunk to the correct file offset
4. Updates the uploaded bytes count in the database

## Impact
Second step in the three-part upload API, enabling actual data transfer.

## Suggested Implementation

### Step 1: Add chunk upload handler to upload_api.rs

Add to `server/src/upload_api.rs`:

```rust
use axum::{
    body::Bytes,
    http::{StatusCode, HeaderMap},
};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

#[derive(Debug, Serialize)]
pub struct ChunkUploadResponse {
    pub status: String,
    pub chunk_index: usize,
    pub bytes_received: usize,
}

/// Upload chunk endpoint - receives individual chunks with checksum verification
pub async fn upload_chunk(
    State(state): State<Arc<UploadState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ChunkUploadResponse>, StatusCode> {
    // Step 1: Extract headers
    let recording_id = headers
        .get("X-Recording-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let file_type = headers
        .get("X-File-Type")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let chunk_index: usize = headers
        .get("X-Chunk-Index")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let expected_checksum = headers
        .get("X-Chunk-Checksum")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Step 2: Verify checksum
    let actual_checksum = crate::upload_utils::calculate_checksum(&body);
    if actual_checksum != expected_checksum {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Step 3: Get file path from database
    let file_path = {
        let db = state.db.lock().await;
        let column = if file_type == "video" { "video_path" } else { "sensor_path" };
        let query = format!("SELECT {} FROM recordings WHERE recording_id = ?", column);

        let path: String = db
            .query_row(&query, params![recording_id], |row| row.get(0))
            .map_err(|_| StatusCode::NOT_FOUND)?;
        PathBuf::from(path)
    };

    // Step 4: Open file and write chunk at offset
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&file_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let offset = (chunk_index * CHUNK_SIZE) as u64;
    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    file.write_all(&body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    file.flush()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Step 5: Update uploaded bytes in database
    let bytes_received = body.len();
    {
        let db = state.db.lock().await;
        let column = if file_type == "video" {
            "video_uploaded_bytes"
        } else {
            "sensor_uploaded_bytes"
        };
        let query = format!(
            "UPDATE recordings SET {} = {} WHERE recording_id = ?",
            column,
            offset + bytes_received as u64
        );

        db.execute(&query, params![recording_id])
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(ChunkUploadResponse {
        status: "received".to_string(),
        chunk_index,
        bytes_received,
    }))
}
```

### Step 2: Add required imports

Ensure these imports are at the top of `upload_api.rs`:

```rust
use axum::body::Bytes;
use axum::http::HeaderMap;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
```

## Headers Required
- `X-Recording-Id`: UUID of the recording
- `X-File-Type`: "video" or "sensor"
- `X-Chunk-Index`: 0-based index of the chunk
- `X-Chunk-Checksum`: SHA-256 hex string of the chunk data

## Error Handling
- 400 Bad Request: Missing headers or checksum mismatch
- 404 Not Found: Recording ID not found
- 500 Internal Server Error: File write failure

## Testing
- Build succeeds: `cargo build`
- Manual test: POST binary data with required headers

## Dependencies
- Issue 040: Implement upload prepare endpoint
- Issue 039: Implement chunk checksum utilities

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 9 for full details.
See `docs/plans/2026-02-05-survey-upload-feature-design.md` - Upload Protocol Step 2.

---
*Created: 2026-02-05*
