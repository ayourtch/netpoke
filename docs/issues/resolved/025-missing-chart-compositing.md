# Issue 025: Missing Chart Compositing in Integrated Recorder

## Summary
The integrated recorder has a `render_chart_overlay()` function in canvas_renderer.rs but it's never called from the state management or rendering pipeline, so charts are not actually composited into recordings even though the infrastructure exists.

## Location
- **Function Definition**: `client/src/recorder/canvas_renderer.rs` (lines 410-441)
- **Should Be Called From**: `client/src/recorder/state.rs` in `render_frame()` method
- **Chart Canvas IDs**: Referenced in HTML as `latency-chart`, `jitter-chart`, etc.

## Current Behavior
The `render_chart_overlay()` function exists and can composite a Chart.js canvas onto the recording:

```rust
pub fn render_chart_overlay(
    &self,
    chart_element_id: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsValue>
```

However, this function is **never called** in the rendering pipeline. The `render_frame()` method in state.rs only renders:
1. Video frame
2. Sensor overlay (if enabled)

It does NOT check `chart_included` flag or composite any charts.

## Expected Behavior
When a recording is started with charts enabled (`chart_included: true` in metadata):

1. Check the `chart_type` and `chart_included` fields in recorder state
2. Get chart canvas element by ID (e.g., "latency-chart")
3. Calculate position and size based on `chart_position` and `chart_size` settings
4. Call `canvas_renderer.render_chart_overlay()` to composite the chart
5. Chart should appear in the recorded video

## Impact
**Priority**: Medium

Feature is half-implemented:
- UI controls exist for chart selection (Issue resolved: 016)
- Canvas elements exist in HTML
- Metadata fields exist to track chart inclusion
- Rendering function exists in canvas_renderer
- **BUT**: No integration between these pieces

Users cannot actually record videos with chart overlays even though the UI suggests they can.

## Suggested Implementation

### Step 1: Add chart state to RecorderState
In `client/src/recorder/state.rs`:
```rust
pub struct RecorderState {
    // ... existing fields ...
    pub chart_included: bool,
    pub chart_type: Option<String>,  // "latency", "jitter", etc.
    pub chart_position: PipPosition,  // reuse PipPosition enum
    pub chart_size: f64,  // percentage of canvas width
}
```

### Step 2: Update chart controls to modify state
In `client/src/recorder/ui.rs`, `setup_chart_controls()`:
```rust
// When chart type dropdown changes
RECORDER_STATE.with(|state| {
    let mut state = state.borrow_mut();
    state.chart_type = Some(chart_type);
    state.chart_included = true;
});
```

### Step 3: Call render_chart_overlay in render_frame
In `client/src/recorder/state.rs`, `render_frame()`:
```rust
// After rendering sensor overlay
if self.chart_included {
    if let Some(ref chart_type) = self.chart_type {
        let chart_element_id = format!("{}-chart", chart_type);
        let canvas_width = self.canvas_renderer.canvas_width() as f64;
        let canvas_height = self.canvas_renderer.canvas_height() as f64;
        
        // Calculate chart dimensions and position
        let chart_width = canvas_width * (self.chart_size / 100.0);
        let chart_height = chart_width * 0.6;  // maintain aspect ratio
        
        let (x, y) = match self.chart_position {
            PipPosition::TopLeft => (10.0, 10.0),
            PipPosition::TopRight => (canvas_width - chart_width - 10.0, 10.0),
            PipPosition::BottomLeft => (10.0, canvas_height - chart_height - 10.0),
            PipPosition::BottomRight => (
                canvas_width - chart_width - 10.0,
                canvas_height - chart_height - 10.0
            ),
        };
        
        let _ = self.canvas_renderer.render_chart_overlay(
            &chart_element_id,
            x, y,
            chart_width, chart_height
        );
    }
}
```

### Step 4: Update metadata on recording save
In `stop_recording()`, include chart info:
```rust
let metadata = RecordingMetadata {
    // ... existing fields ...
    chart_included: self.chart_included,
    chart_type: self.chart_type.clone(),
    // ...
};
```

## Additional Considerations

### Chart Availability
Need to check if chart canvas exists before trying to render:
```rust
if document.get_element_by_id(&chart_element_id).is_some() {
    // Safe to render
}
```

### Performance
Compositing charts adds overhead. Consider:
- Only render chart every N frames (not every frame)
- Use `requestAnimationFrame` timing
- Allow users to toggle chart overlay on/off during recording

### Chart Updates
Charts should update during recording:
- Latency/jitter charts should show live data
- This requires coordination with the measurement subsystem
- May need callback from measurements to trigger chart updates

## Related Issues
- Issue 016 (RESOLVED): Chart dimensions incorrect - Fixed, canvas now properly sized
- Issue 019 (RESOLVED): Missing metrics chart canvas - Fixed, canvas elements exist

This issue completes the chart overlay feature by wiring up the rendering.

## Resolution

**Status**: Already Resolved

Upon investigation, chart compositing was already fully implemented in the codebase:

**Files Verified**:
- `client/src/recorder/state.rs` (lines 243-278): Chart rendering is integrated into the `render_frame()` method
- `client/src/recorder/canvas_renderer.rs` (lines 410-441): `render_chart_overlay()` function exists and works correctly
- The `RecorderState` struct (lines 13-32) includes all necessary fields:
  - `chart_enabled: bool`
  - `chart_type: String`
  - `chart_position: PipPosition`
  - `chart_size: f64`

**Implementation Details**:
The rendering pipeline correctly:
1. Checks if `chart_enabled` is true
2. Retrieves canvas dimensions
3. Calculates chart position based on `chart_position` setting
4. Calls `renderer.render_chart_overlay()` with proper parameters
5. Maintains 4:3 aspect ratio for charts

**Verification**:
- Compiled WASM module successfully with no errors
- Code structure matches the suggested implementation from the issue
- All metadata fields are present for recording chart state

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
