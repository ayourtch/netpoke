# Issue 051: Fix Upload 404 Error When Database Unavailable

## Summary
When database initialization fails, upload routes are not registered, causing a confusing 404 error ("Prepare failed: 404") when users try to upload video+sensor data.

## Location
- File: `server/src/main.rs` (lines 144-156) - upload route registration
- File: `server/src/upload_api.rs` - upload endpoints

## Current Behavior
1. If database initialization fails, `db = None`
2. Upload routes are conditionally created only when `db.is_some()`
3. Routes are not registered → 404 Not Found for any `/api/upload/*` request
4. User sees: "⚠️ Upload failed: Prepare failed: 404"

This is confusing because:
- The survey session appears to work (traceroute, video recording)
- Upload fails with an unhelpful 404 error
- User has no idea that the database is not configured

## Expected Behavior
1. Upload routes should always be registered
2. If database is unavailable, return a clear error message:
   - Status: 503 Service Unavailable
   - Message: "Upload feature is unavailable - database not configured"
3. Client should display this helpful message to the user

## Impact
- Users get confusing 404 errors instead of actionable feedback
- Survey sessions can still record video but uploads will always fail
- No indication that database configuration is the issue

## Root Cause Analysis
The upload routes are created inside a `db.as_ref().map()` closure, which means they're only registered when the database is available. This is defensive but unhelpful - it silently disables the feature without user feedback.

## Suggested Implementation

### Step 1: Modify UploadState to handle optional database

Change `UploadState` in `upload_api.rs` to have optional database:

```rust
pub struct UploadState {
    pub db: Option<DbConnection>,  // Changed from DbConnection
    pub storage_base_path: String,
}
```

### Step 2: Always register upload routes in main.rs

```rust
let upload_state = Arc::new(upload_api::UploadState {
    db: db.clone(),  // Pass Option<DbConnection>
    storage_base_path: storage_base_path.clone(),
});

let upload_routes = Router::new()
    .route("/api/upload/prepare", post(upload_api::prepare_upload))
    .route("/api/upload/chunk", post(upload_api::upload_chunk))
    .route("/api/upload/finalize", post(upload_api::finalize_upload))
    .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
    .with_state(upload_state);
```

### Step 3: Update upload handlers to check for database

In each handler, check if database is available:

```rust
pub async fn prepare_upload(
    State(state): State<Arc<UploadState>>,
    Json(req): Json<PrepareUploadRequest>,
) -> Result<Json<PrepareUploadResponse>, (StatusCode, Json<UploadError>)> {
    // Check if database is available
    let db = match &state.db {
        Some(db) => db,
        None => {
            tracing::warn!("Upload prepare failed: database not configured");
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(UploadError {
                    error: "Upload feature is unavailable - database not configured".to_string(),
                })
            ));
        }
    };
    // ... rest of handler
}
```

### Step 4: Update client to display better error messages

In `nettest.html`, parse error response body for better messages:

```javascript
if (!prepareResp.ok) {
    let errorMsg = `Prepare failed: ${prepareResp.status}`;
    try {
        const errorData = await prepareResp.json();
        if (errorData.error) {
            errorMsg = errorData.error;
        }
    } catch (e) {
        // Not JSON, use status-based message
        if (prepareResp.status === 503) {
            errorMsg = 'Upload service unavailable - server may not be fully configured';
        }
    }
    throw new Error(errorMsg);
}
```

## Testing
- Start server without database configured
- Attempt upload → should see helpful error message
- Start server with database configured
- Attempt upload → should work as before

## Dependencies
None - this is a bug fix

---
*Created: 2026-02-05*

---

## Resolution

### Changes Made

1. **Modified `upload_api.rs`:**
   - Changed `UploadState.db` from `DbConnection` to `Option<DbConnection>`
   - Added `UploadError` struct for structured error responses with the `error` field
   - Updated all three handlers (`prepare_upload`, `upload_chunk`, `finalize_upload`) to:
     - Check for database availability at the start
     - Return `503 Service Unavailable` with clear message if database not configured
     - Return structured JSON error responses instead of bare status codes

2. **Modified `main.rs`:**
   - Changed upload routes from conditional (`Option<Router>`) to always registered
   - Upload routes now take `Option<DbConnection>` and handle None case gracefully
   - Removed all `if let Some(upload) = upload_routes` checks in router merging

3. **Modified `nettest.html`:**
   - Updated error handling to parse JSON error responses from server
   - Display helpful error messages to users instead of raw HTTP status codes
   - Better fallback messages for 503 (service unavailable) and 404 (session not found)

### Files Modified
- `server/src/upload_api.rs` - Handler logic and error types
- `server/src/main.rs` - Route registration
- `server/static/nettest.html` - Client-side error handling

### Verification
- Server builds successfully with `cargo build --package netpoke-server`
- No new warnings related to upload functionality
- Error messages are now structured JSON that clients can parse and display

### Before/After

**Before (confusing):**
```
⚠️ Upload failed: Prepare failed: 404
```

**After (helpful):**
```
⚠️ Upload failed: Upload feature is unavailable - database not configured
```
or
```
⚠️ Upload failed: Session not found: <session-id>
```

---
*Resolved: 2026-02-05*
