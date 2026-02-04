# Issue 016: Chart Overlay Dimensions Incorrectly Calculated

## Summary
The chart overlay dimensions in `render_frame()` use hardcoded base sizes that don't match the actual Chart.js canvas dimensions. This could result in distorted or incorrectly sized chart overlays in recordings.

## Location
- File: `client/src/recorder/state.rs`
- Function: `render_frame()` 
- Lines: 184-186

## Current Behavior
```rust
// Calculate chart dimensions
let chart_width = 300.0 * self.chart_size;
let chart_height = 200.0 * self.chart_size;
```

Uses hardcoded 300x200 base dimensions, then scales by chart_size (which is a percentage like 0.20 for 20%).

## Expected Behavior
The chart dimensions should be based on:
1. The actual canvas dimensions of the chart being captured, OR
2. A percentage of the recording canvas dimensions (as the design doc suggests)

Per the design doc:
```
- Size: 10-30% of video width
```

So the calculation should be:
```rust
let chart_width = canvas_width * self.chart_size;
// Maintain aspect ratio of actual chart or use 3:2 ratio
let chart_height = chart_width * 0.667; // or actual chart aspect ratio
```

## Impact
- **Priority: Low**
- Chart overlay may appear incorrectly sized
- On high-resolution recordings, the 60x40 pixel (at 20% of 300x200) overlay would be tiny
- On low-resolution recordings, it might be too large
- Chart aspect ratio may not match actual chart

## Suggested Implementation
1. **Use percentage of recording canvas:**
```rust
if self.chart_enabled {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(canvas_element) = document.get_element_by_id("recordingCanvas") {
                if let Ok(canvas) = canvas_element.dyn_into::<web_sys::HtmlCanvasElement>() {
                    let canvas_width = canvas.width() as f64;
                    let canvas_height = canvas.height() as f64;

                    // Chart width is percentage of canvas width
                    let chart_width = canvas_width * self.chart_size;
                    
                    // Get actual chart canvas to determine aspect ratio
                    if let Some(chart_canvas) = document.get_element_by_id(&self.chart_type) {
                        if let Ok(chart) = chart_canvas.dyn_into::<web_sys::HtmlCanvasElement>() {
                            let chart_aspect = chart.height() as f64 / chart.width() as f64;
                            let chart_height = chart_width * chart_aspect;
                            
                            // Calculate position...
                        }
                    }
                }
            }
        }
    }
}
```

2. **Or use fixed aspect ratio but scale to canvas:**
```rust
let chart_width = canvas_width * self.chart_size;
let chart_height = chart_width * 0.6; // 5:3 aspect ratio (common for charts)
```

---
*Created: 2026-02-04*
