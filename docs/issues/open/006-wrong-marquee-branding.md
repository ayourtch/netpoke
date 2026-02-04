# Issue 006: Wrong Marquee Branding URL

## Summary
The canvas renderer draws a scrolling marquee with the URL "https://stdio.be/cast" which is from the standalone camera app. It should be updated to reflect netpoke branding or removed entirely.

## Location
- File: `client/src/recorder/canvas_renderer.rs`
- Function: `draw_marquee()`
- Line: 183

## Current Behavior
```rust
fn draw_marquee(&self, canvas_width: f64) -> Result<(), JsValue> {
    let text = "https://stdio.be/cast - record your own screencast";
    // ...
}
```

The marquee displays a URL pointing to an external site (stdio.be/cast) which is unrelated to netpoke.

## Expected Behavior
Either:
1. Update to netpoke branding: "https://netpoke.com - Network Measurement Tool"
2. Remove the marquee entirely if not desired for the network testing context
3. Make the marquee text configurable

## Impact
- **Priority: Low**
- Branding inconsistency - recordings show wrong product URL
- Confusing for users who see an unrelated URL in their recordings
- May appear unprofessional or like the feature was copied without proper adaptation

## Suggested Implementation
**Option 1 - Update branding:**
```rust
fn draw_marquee(&self, canvas_width: f64) -> Result<(), JsValue> {
    let text = "NetPoke - Network Measurement Tool";
    // or
    let text = "https://netpoke.com - Network Measurement";
    // ...
}
```

**Option 2 - Remove the marquee:**
Remove the `draw_marquee()` calls from:
- `render_camera()` (line 68)
- `render_screen()` (line 93)  
- `render_combined()` (line 177)

And optionally remove the `draw_marquee()` function entirely.

**Option 3 - Make configurable:**
Pass marquee text as a parameter or make it configurable through RecorderState.

---
*Created: 2026-02-04*
