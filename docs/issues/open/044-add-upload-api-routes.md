# Issue 044: Add Upload API Routes

## Summary
Register the upload API endpoints (prepare, chunk, finalize) with the Axum router.

## Location
- File: `server/src/main.rs`

## Current Behavior
Upload API handlers exist but are not exposed as HTTP routes.

## Expected Behavior
Three routes registered:
1. `POST /api/upload/prepare` - Initiate/resume upload
2. `POST /api/upload/chunk` - Upload individual chunks
3. `POST /api/upload/finalize` - Complete upload

## Impact
Enables client-side code to call the upload API endpoints.

## Suggested Implementation

### Step 1: Create UploadState and router

In `server/src/main.rs`, after service initialization:

```rust
// Upload API routes (only if database is available)
let app = if let Some(ref db) = db {
    let upload_state = Arc::new(crate::upload_api::UploadState {
        db: db.clone(),
        storage_base_path: config.storage.base_path.clone(),
    });

    let upload_routes = axum::Router::new()
        .route("/api/upload/prepare", axum::routing::post(crate::upload_api::prepare_upload))
        .route("/api/upload/chunk", axum::routing::post(crate::upload_api::upload_chunk))
        .route("/api/upload/finalize", axum::routing::post(crate::upload_api::finalize_upload))
        .with_state(upload_state);

    app.merge(upload_routes)
} else {
    app
};
```

### Step 2: Configure body size limit for chunk uploads

Add body size limit to allow 1MB+ uploads:

```rust
use axum::extract::DefaultBodyLimit;

let upload_routes = axum::Router::new()
    .route("/api/upload/prepare", axum::routing::post(crate::upload_api::prepare_upload))
    .route("/api/upload/chunk", axum::routing::post(crate::upload_api::upload_chunk))
    .route("/api/upload/finalize", axum::routing::post(crate::upload_api::finalize_upload))
    .layer(DefaultBodyLimit::max(2 * 1024 * 1024)) // 2MB to be safe
    .with_state(upload_state);
```

## Route Specifications

### POST /api/upload/prepare
- **Body:** JSON with session_id, recording_id, sizes, device_info
- **Response:** JSON with recording_id and existing chunk checksums

### POST /api/upload/chunk
- **Headers:** X-Recording-Id, X-File-Type, X-Chunk-Index, X-Chunk-Checksum
- **Body:** Binary chunk data (up to 1MB)
- **Response:** JSON with status and bytes received

### POST /api/upload/finalize
- **Body:** JSON with recording_id and final checksums
- **Response:** JSON with verification status

## Testing
- Build succeeds: `cargo build`
- Server starts and routes are accessible
- Manual test: `curl -X POST http://localhost:8080/api/upload/prepare ...`

## Dependencies
- Issue 040: Implement upload prepare endpoint
- Issue 041: Implement upload chunk endpoint
- Issue 042: Implement upload finalize endpoint
- Issue 043: Initialize services in main.rs

## Reference
See `docs/plans/2026-02-05-survey-upload-implementation.md` - Task 12 for full details.

---
*Created: 2026-02-05*
