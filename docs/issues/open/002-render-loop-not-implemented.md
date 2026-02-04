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
