# Issue 002: Recording Render Loop Not Properly Implemented

## Summary
The `start_render_loop()` method in RecorderState returns immediately without starting an actual animation loop. The design calls for a 30 FPS render loop to composite video frames, PiP overlays, charts, and sensor data.

## Location
- File: `client/src/recorder/state.rs`
- Function: `start_render_loop()`
- Lines: 138-148

## Current Behavior
```rust
fn start_render_loop(&mut self) -> Result<(), JsValue> {
    use crate::recorder::utils::log;

    log("[Recorder] Starting render loop");

    // We'll use a simple approach: export a render function and call it from JavaScript
    // For now, return Ok - the actual rendering will be driven by a JavaScript setInterval
    // This is simpler than managing Rust closures with requestAnimationFrame

    Ok(())
}
```
The function logs a message and immediately returns without setting up any rendering.

## Expected Behavior
Per the design document and standalone camera implementation (`tmp/camera-standalone-for-cross-check/src/app.rs` lines 288-362):
- The render loop should use `setInterval` with a 33ms interval (~30 FPS)
- Each frame should:
  1. Render the main video source (camera/screen/combined)
  2. Render PiP overlay if in combined mode
  3. Render chart overlay if enabled
  4. Render sensor overlay if enabled
  5. Render compass if camera direction is available

The standalone camera achieves this with:
```rust
let handle = window.set_interval_with_callback_and_timeout_and_arguments_0(
    closure.as_ref().unchecked_ref(),
    33,
)?;
closure.forget();
self.render_interval_handle = Some(handle);
```

## Impact
- **Priority: Critical**
- Recordings will be blank/empty since no frames are rendered to the recording canvas
- The render_frame() method exists but is never called in a loop
- Users will get non-functional recordings

## Suggested Implementation
1. Create a JavaScript-driven render loop by modifying `server/static/nettest.html`:
   ```javascript
   let renderInterval = null;
   
   function startRecorderRenderLoop() {
       if (renderInterval) return;
       renderInterval = setInterval(() => {
           if (typeof recorder_render_frame === 'function') {
               recorder_render_frame();
           }
       }, 33);  // ~30 FPS
   }
   
   function stopRecorderRenderLoop() {
       if (renderInterval) {
           clearInterval(renderInterval);
           renderInterval = null;
       }
   }
   ```

2. Call `startRecorderRenderLoop()` when recording starts and `stopRecorderRenderLoop()` when it stops

3. Alternatively, implement the full Rust closure pattern from the standalone camera:
   - Copy the implementation from `tmp/camera-standalone-for-cross-check/src/app.rs` lines 288-362
   - Adapt it to work with RecorderState instead of AppState

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*

## Resolution

**Status**: Already resolved in current codebase, confirmed with code review and build verification.

### What Was Found
The render loop was already properly implemented:
1. The `recorder_render_frame()` function exists in `client/src/recorder/ui.rs` with `#[wasm_bindgen]` export (lines 33-38)
2. The HTML (`server/static/nettest.html`) already has a render loop using `requestAnimationFrame` (lines 2580-2588)
3. The function is correctly imported from the WASM module (line 2566)

### Actions Taken
No code changes were needed for the core render loop functionality. However, during investigation:
1. Verified that `recorder_render_frame()` is exported from the WASM module
2. Built the client WASM module to confirm the export is present in the generated JS wrapper
3. The render loop calls `render_frame()` on the RecorderState, which composites video, PiP, charts, and sensors

### Related Changes
As part of Issue 013 (UI state management), added metrics display updates to the render loop:
- Call `update_recording_metrics()` from `render_frame()` to show duration, frames, and estimated size
- Added metrics display HTML elements
- Added status badge updates

The render loop is functional and properly integrated.
