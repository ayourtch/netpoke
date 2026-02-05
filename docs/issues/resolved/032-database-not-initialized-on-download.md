# Issue 032: Database Not Initialized Error on Download After Page Refresh

## Summary
When refreshing the page, existing recordings are visible in the IndexedDB list, but attempting to download video or motion data fails with "Database not initialized" error.

## Location
- File: `client/src/lib.rs`
- Functions: `download_video()` (line ~2037) and `download_motion_data()` (line ~2084)
- Related: `server/static/lib/recorder/indexed_db.js` (module-level `db` variable)

## Current Behavior
1. User refreshes page
2. `refreshRecordingsList()` in `nettest.html` opens IndexedDB directly and displays recordings
3. User clicks "Download Video" or "Download Motion" button
4. `download_video()` or `download_motion_data()` calls `getRecording()` from `indexed_db.js`
5. `getRecording()` throws "Database not initialized" because module's `db` variable is `null`

The root cause is that `download_video()` and `download_motion_data()` directly call `getRecording()` without first initializing the database via `openDb()`.

```rust
// Current code in lib.rs - download_video()
pub async fn download_video(id: String) -> Result<(), JsValue> {
    // ...
    // Get recording from IndexedDB - directly calls getRecording without openDb()
    let recording_js = recorder::storage::getRecording(&id).await?;
    // ...
}
```

Compare with `delete_recording_by_id()` which correctly initializes the database:

```rust
// Correct pattern in lib.rs - delete_recording_by_id()
pub async fn delete_recording_by_id(id: String) -> Result<(), JsValue> {
    // ...
    // Delete from IndexedDB - correctly calls IndexedDbWrapper::open() first
    let db = IndexedDbWrapper::open().await?;
    db.delete_recording(&id).await?;
    // ...
}
```

## Expected Behavior
Downloads should work correctly after page refresh. The database should be initialized before accessing recordings.

## Impact
**High** - Users cannot download their recorded videos or motion data after refreshing the page. They would need to create a new recording first (which calls `openDb()`) before downloads work.

## Suggested Implementation

Add `openDb()` call before `getRecording()` in both functions:

```rust
#[wasm_bindgen]
pub async fn download_video(id: String) -> Result<(), JsValue> {
    use recorder::utils::log;

    log(&format!("[Recorder] Downloading video: {}", id));

    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    // Initialize database before accessing
    recorder::storage::openDb().await?;

    // Get recording from IndexedDB
    let recording_js = recorder::storage::getRecording(&id).await?;
    // ... rest of function
}

#[wasm_bindgen]
pub async fn download_motion_data(id: String) -> Result<(), JsValue> {
    use recorder::utils::log;

    log(&format!("[Recorder] Downloading motion data: {}", id));

    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    // Initialize database before accessing
    recorder::storage::openDb().await?;

    // Get recording from IndexedDB
    let recording_js = recorder::storage::getRecording(&id).await?;
    // ... rest of function
}
```

Note: `openDb()` is idempotent - calling it multiple times is safe as it simply reopens the database connection.

## Related Issues
- Issue 012 (resolved): Missing download functions - functions were added but without proper database initialization
- Issue 026 (resolved): Missing recordings list implementation - list uses inline IndexedDB, not the module

## Resolution

**Resolved: 2026-02-05**

Added `openDb()` initialization call before `getRecording()` in both `download_video()` and `download_motion_data()` functions in `client/src/lib.rs`.

### Changes Made:

1. **In `client/src/lib.rs` - `download_video()` function (line ~2045)**:
   - Added `recorder::storage::openDb().await?;` before calling `getRecording()`
   - Added comment `// Initialize database before accessing (Issue 032)`

2. **In `client/src/lib.rs` - `download_motion_data()` function (line ~2095)**:
   - Added `recorder::storage::openDb().await?;` before calling `getRecording()`
   - Added comment `// Initialize database before accessing (Issue 032)`

### Verification:
- Code compiles successfully with `cargo check --package netpoke-client --target wasm32-unknown-unknown`
- The fix follows the same pattern used in `delete_recording_by_id()` which already worked correctly
- `openDb()` is idempotent, so repeated calls are safe and don't cause issues

### Root Cause Analysis:
The issue occurred because `download_video()` and `download_motion_data()` were directly calling the JavaScript `getRecording()` function without first initializing the IndexedDB connection. The JavaScript module (`indexed_db.js`) has a module-level `db` variable that must be set by calling `openDb()` first. After page refresh, this variable was `null` because:
1. The `refreshRecordingsList()` function in `nettest.html` uses its own inline IndexedDB access (not the `indexed_db.js` module)
2. No WASM function that calls `openDb()` had been invoked yet

---
*Created: 2026-02-05*
*Resolved: 2026-02-05*
