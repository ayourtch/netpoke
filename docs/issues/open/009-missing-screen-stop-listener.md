# Issue 009: Missing Screen Share Stop Listener

## Summary
The standalone camera app sets up a listener for when the user stops screen sharing via the browser's built-in "Stop sharing" button. The netpoke implementation does not use this listener, so recordings continue even after screen sharing is stopped.

## Location
- File: `client/src/recorder/media_streams.rs` - `add_screen_stop_listener()` function exists but is unused
- File: `client/src/recorder/state.rs` - `start_recording()` should call it
- Reference: `tmp/camera-standalone-for-cross-check/src/media_streams.rs` lines 73-82

## Current Behavior
The function exists in `client/src/recorder/media_streams.rs`:
```rust
pub fn add_screen_stop_listener(stream: &MediaStream, callback: Box<dyn Fn()>) -> Result<(), JsValue> {
    let tracks = stream.get_video_tracks();
    if tracks.length() > 0 {
        let track = MediaStreamTrack::from(tracks.get(0));
        let closure = Closure::wrap(callback as Box<dyn Fn()>);
        track.add_event_listener_with_callback("ended", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    Ok(())
}
```

But it is never called from `start_recording()` in state.rs.

## Expected Behavior
When the user clicks "Stop sharing" in the browser's screen share UI:
1. The recording should automatically stop
2. The video should be saved
3. The UI should update to reflect stopped state

The standalone camera achieves this by attaching the listener after getting the screen stream.

## Impact
- **Priority: Medium**
- If user stops screen sharing via browser UI, recording continues with blank/frozen video
- User may not realize recording is still happening
- Results in confusing recordings with missing content
- Poor user experience

## Suggested Implementation
In `client/src/recorder/state.rs` `start_recording()`, after getting the screen stream for Screen or Combined mode:

```rust
// After getting screen stream:
if matches!(self.source_type, SourceType::Screen | SourceType::Combined) {
    if let Some(ref screen_stream) = self.screen_stream {
        // Clone state reference for the closure
        let stop_callback = Box::new(move || {
            // This callback is called when user clicks "Stop sharing"
            crate::recorder::utils::log("[Recorder] Screen sharing stopped by user");
            // Trigger stop recording
            // Note: This requires access to stop_recording(), which is tricky from a closure
            // May need to use a JavaScript callback instead
        });
        
        crate::recorder::media_streams::add_screen_stop_listener(
            screen_stream,
            stop_callback
        )?;
    }
}
```

**Alternative approach using JavaScript:**
Set up a global callback in JavaScript that the screen stop event can trigger:
```javascript
window.onScreenShareStopped = function() {
    if (typeof stop_recording === 'function') {
        stop_recording();
    }
};
```

And have the Rust closure call this JavaScript function.

---
*Created: 2026-02-04*
