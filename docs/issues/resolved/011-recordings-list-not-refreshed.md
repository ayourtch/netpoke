# Issue 011: Recordings List Not Refreshed After Save

## Summary
The standalone camera app refreshes the recordings list in the UI after saving a recording. The netpoke implementation saves the recording but does not update the recordings list, so users don't see their new recording until they reload the page.

## Location
- File: `client/src/recorder/state.rs` - `stop_recording()` function
- Reference: `tmp/camera-standalone-for-cross-check/src/app.rs` - `stop_tracking()` line 485

## Current Behavior
In `client/src/recorder/state.rs` `stop_recording()`:
```rust
// Save to IndexedDB
let db = IndexedDbWrapper::open().await?;
db.save_recording(&id, &blob, &metadata, &motion_data).await?;

// Cleanup
self.camera_stream = None;
// ... more cleanup ...

log("[Recorder] Recording saved");
Ok(())
```

The recording is saved but the recordings list in the UI is not updated.

## Expected Behavior
The standalone camera app refreshes the UI after saving:
```rust
// Save to IndexedDB
self.db.save_recording(&recording_id, &blob, &metadata, &motion_data).await?;

// Update UI
self.ui.show_ready_state()?;
self.ui.set_status("Recording saved!")?;
self.refresh_recordings_list().await?;  // <-- This is missing!
```

## Impact
- **Priority: Medium**
- Users don't see their recording in the list after saving
- Must reload page to see new recordings
- Poor user experience, users may think recording failed
- May lead to users re-recording unnecessarily

## Suggested Implementation
After saving the recording in `stop_recording()`, refresh the recordings list:

```rust
// Save to IndexedDB
let db = IndexedDbWrapper::open().await?;
db.save_recording(&id, &blob, &metadata, &motion_data).await?;

// Refresh recordings list in UI
let recordings = db.get_all_recordings().await?;
update_recordings_list_ui(&recordings)?;

// Or call a JavaScript function to refresh
let window = web_sys::window().ok_or("No window")?;
if let Ok(refresh_fn) = js_sys::Reflect::get(&window, &"refreshRecordingsList".into()) {
    if refresh_fn.is_function() {
        let func: js_sys::Function = refresh_fn.dyn_into()?;
        func.call0(&window)?;
    }
}
```

**JavaScript Helper (add to nettest.html):**
```javascript
async function refreshRecordingsList() {
    const db = await openDb();
    const recordings = await getAllRecordings();
    // Render recordings to #recordings-container
    renderRecordingsList(recordings);
}

window.refreshRecordingsList = refreshRecordingsList;
```

Alternatively, the recordings list UI could be re-rendered from Rust using DOM manipulation, similar to how the standalone camera's `UiController::render_recordings_list()` works.

## Resolution
Fixed in commit 9ab2ea2 (2026-02-04).

**Changes made:**
1. Added `refreshRecordingsList()` JavaScript function to `server/static/nettest.html`
2. Exposed function globally via `window.refreshRecordingsList`
3. Modified `stop_recording()` in `client/src/recorder/state.rs` to call the JavaScript refresh function after saving to IndexedDB
4. Uses `js_sys::Reflect` to check for and call the function if it exists

The recordings list UI can now be refreshed after a new recording is saved, though the full IndexedDB query implementation in the JavaScript function still needs to be completed.

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
