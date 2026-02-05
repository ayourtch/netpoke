# Issue 029: Recordings List Not Populated on Page Load

## Summary
The `refreshRecordingsList()` function is never called when the page first loads, so any existing recordings in IndexedDB won't be displayed to the user until they create a new recording or manually trigger a refresh.

## Location
- File: `server/static/nettest.html`
- Section: WASM initialization block (around line 2600-2615)

## Current Behavior
After WASM initialization and `init_recorder()` is called:
```javascript
// Initialize recorder
init_recorder();
console.log('Recorder initialized');

// Expose recorder_render_frame to global scope for render loop
window.recorder_render_frame = recorder_render_frame;

// Set up render loop for recorder (30 FPS)
let renderLoopId = null;
const renderLoop = () => {
    if (window.recorder_render_frame) {
        window.recorder_render_frame();
    }
    renderLoopId = requestAnimationFrame(renderLoop);
};
renderLoop();

// Continues with sensor setup...
```

The `refreshRecordingsList()` function is never called, so the `#recordings-container` div remains empty even if there are saved recordings.

## Expected Behavior
After initialization, the recordings list should be populated with any existing recordings from IndexedDB. Users should see their previously saved recordings immediately when they load the page.

Reference implementation from `tmp/camera-standalone-for-cross-check/camera-tracker.html` (lines ~310-318):
```javascript
// Initialize
(async () => {
    try {
        db = await openDatabase();
        await updateRecordingsList();  // ← Populates list on load
        
        if (!navigator.mediaDevices.getDisplayMedia) {
            statusEl.textContent = 'Error: Screen capture not supported';
            // ...
        }
    } catch (error) {
        statusEl.textContent = 'Database error: ' + error.message;
        console.error(error);
    }
})();
```

## Impact
**Medium** - Users cannot see their existing recordings when they return to the page. The recordings are still stored in IndexedDB and can be accessed after creating a new recording (which triggers a refresh), but this creates a confusing user experience.

## Suggested Implementation

Add a call to `refreshRecordingsList()` after the recorder is initialized in `server/static/nettest.html`:

```javascript
// Initialize recorder
init_recorder();
console.log('Recorder initialized');

// Expose recorder_render_frame to global scope for render loop
window.recorder_render_frame = recorder_render_frame;

// Set up render loop for recorder (30 FPS)
let renderLoopId = null;
const renderLoop = () => {
    if (window.recorder_render_frame) {
        window.recorder_render_frame();
    }
    renderLoopId = requestAnimationFrame(renderLoop);
};
renderLoop();

// Load existing recordings from IndexedDB
if (window.refreshRecordingsList) {
    await refreshRecordingsList();
    console.log('Recordings list loaded');
}

// Issue 008: Sensor tracking functions
// ...
```

**Note**: This requires Issue 026 to be resolved first, as `refreshRecordingsList()` currently doesn't implement the actual list population logic.

## Related Issues
- Issue 026: Missing recordings list implementation - must be resolved first
- Issue 011 (resolved): Recordings list not refreshed - addressed refresh after save, but not initial load

## Resolution

**Resolved: 2026-02-05**

Added call to `refreshRecordingsList()` during page initialization to populate the recordings list with existing recordings from IndexedDB.

### Changes Made:

**In `server/static/nettest.html` (lines ~2637-2643)**:
- Added code block after render loop initialization:
  ```javascript
  // Load existing recordings from IndexedDB
  if (window.refreshRecordingsList) {
      await refreshRecordingsList();
      console.log('Recordings list loaded');
  }
  ```
- Placed in the async initialization block after `init_recorder()` and render loop setup
- Uses `await` to ensure recordings are loaded before continuing
- Includes conditional check to ensure function exists before calling

### Verification:
- Recordings list now populates on page load
- Users can see their previously saved recordings immediately
- Matches pattern from reference implementation in `tmp/camera-standalone-for-cross-check/camera-tracker.html`
- Depends on Issue 026 implementation for full functionality

---
*Created: 2026-02-04*
