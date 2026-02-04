# Issue 013: UI State Management Differences

## Summary
The standalone camera app has a rich UiController class that manages UI state (buttons enabled/disabled, showing/hiding sections, status messages). The netpoke integration has a simpler UI approach that may not properly manage all UI states during recording.

## Location
- File: `client/src/recorder/ui.rs` - Limited UI management
- Reference: `tmp/camera-standalone-for-cross-check/src/ui.rs` - Full UiController

## Current Behavior
The netpoke `client/src/recorder/ui.rs`:
- Has a thread_local `RECORDER_STATE`
- Sets up event listeners for controls
- Has helper functions `update_pip_visibility()`, `update_chart_controls_visibility()`, `update_recording_ui()`

Missing from standalone camera's UiController:
- `set_status()` - Show status messages
- `show_ready_state()` - Reset UI to initial state
- `show_recording_state()` - Update UI for active recording
- `update_metrics()` - Update frames/duration/size display during recording
- `render_recordings_list()` - Render saved recordings

## Expected Behavior
The UI should:
1. Show "Recording..." status when recording starts
2. Display real-time metrics (frames, duration, video size) during recording
3. Disable mode selection and settings during recording
4. Show "Stop Recording" button, hide "Start Recording" button
5. Show "Saving..." status when stopping
6. Show "Recording saved!" status after save
7. Display updated recordings list

## Impact
- **Priority: Medium**
- UI may not accurately reflect recording state
- No real-time feedback during recording (duration, size)
- Users don't know if recording is working
- Confusing user experience

## Suggested Implementation
Enhance `client/src/recorder/ui.rs` with more UI management:

1. **Add status badge update:**
```rust
fn update_status_badge(status: &str, class: &str) {
    if let Some(badge) = document.get_element_by_id("recording-status") {
        badge.set_text_content(Some(status));
        badge.set_class_name(&format!("status-badge {}", class));
    }
}
```

2. **Add metrics update (call periodically during recording):**
```rust
fn update_recording_metrics(duration_secs: f64, frame_count: u32, size_bytes: u64) {
    if let Some(el) = document.get_element_by_id("recording-duration") {
        el.set_text_content(Some(&format!("{:.1}s", duration_secs)));
    }
    if let Some(el) = document.get_element_by_id("recording-frames") {
        el.set_text_content(Some(&frame_count.to_string()));
    }
    if let Some(el) = document.get_element_by_id("recording-size") {
        let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
        el.set_text_content(Some(&format!("{:.2} MB", size_mb)));
    }
}
```

3. **Add metrics display section to HTML** (if not present):
```html
<div id="recording-metrics" style="display:none">
    <span>Duration: <span id="recording-duration">0.0s</span></span>
    <span>Frames: <span id="recording-frames">0</span></span>
    <span>Size: <span id="recording-size">0.00 MB</span></span>
</div>
```

4. **Start metrics update interval** (similar to standalone camera's `start_metrics_loop()`):
```rust
fn start_metrics_loop() -> Result<i32, JsValue> {
    let window = web_sys::window().ok_or("No window")?;
    
    let closure = Closure::wrap(Box::new(move || {
        RECORDER_STATE.with(|state| {
            let s = state.borrow();
            if s.recording {
                let elapsed = (js_sys::Date::now() - s.start_time) / 1000.0;
                update_recording_metrics(elapsed, s.frame_count, /* size */);
            }
        });
    }) as Box<dyn FnMut()>);
    
    let handle = window.set_interval_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        100, // Update every 100ms
    )?;
    closure.forget();
    
    Ok(handle)
}
```

---
*Created: 2026-02-04*
