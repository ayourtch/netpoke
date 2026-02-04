# Camera Recording Integration Design

**Date:** 2026-02-04
**Status:** Design - Ready for Implementation

## Overview

Integrate the camera/screen recording functionality from the standalone `camera` WASM project into the netpoke client. This allows users to record network latency tests with video (camera/screen/both), sensor data overlay, and composited latency graphs all captured in a single recording.

## Goals

- Record network tests with camera, screen sharing, or both (PiP mode)
- Capture real-time latency graphs composited into the video
- Include sensor data overlay (GPS, orientation, acceleration, compass)
- Save recordings to IndexedDB with test metadata
- Maintain all existing camera app features (sensor tracking, PiP controls, marquee)
- No impact on network test accuracy or performance

## User Workflow

1. User opens netpoke and starts network tests
2. User expands "Recording" panel in UI
3. User selects recording mode (camera/screen/combined)
4. User configures overlays (PiP position, chart position/size, sensors on/off)
5. User clicks "Start Recording" (permissions prompt on first use)
6. Recording captures:
   - Video feed (camera/screen/both)
   - Real-time updating latency graphs
   - Sensor overlay with GPS/orientation/acceleration data
   - Compass indicator
7. User clicks "Stop Recording"
8. Recording saved to IndexedDB with test metadata
9. User can download video (.webm) or sensor data (.json)

## Architecture

### Integration Approach

**Merge camera WASM into netpoke-client crate** - Create a unified WASM binary by copying camera's Rust modules into netpoke client source as a new `recorder/` submodule.

**Advantages:**
- Single WASM binary (simpler deployment)
- Shared types and utilities
- Easier maintenance (one build process)
- No coordination between multiple WASM modules

### Module Structure

```
netpoke/client/src/
├── lib.rs                    # Existing + recorder exports
├── measurements.rs           # Existing
├── signaling.rs             # Existing
├── webrtc.rs                # Existing
└── recorder/                # New module
    ├── mod.rs               # Public API
    ├── state.rs             # Recording state (adapted from camera app.rs)
    ├── ui.rs                # Recording panel controls
    ├── canvas_renderer.rs   # Video compositing + overlays
    ├── media_streams.rs     # Camera/screen capture
    ├── media_recorder.rs    # MediaRecorder wrapper
    ├── storage.rs           # IndexedDB wrapper
    ├── sensors.rs           # Sensor tracking
    ├── types.rs             # Recording types
    └── utils.rs             # Utilities
```

### Files to Copy from Camera

| Camera File | Netpoke Destination | Changes |
|------------|---------------------|---------|
| `src/app.rs` | `recorder/state.rs` | Rename, adapt state management |
| `src/canvas_renderer.rs` | `recorder/canvas_renderer.rs` | Add chart compositing |
| `src/media_streams.rs` | `recorder/media_streams.rs` | Minimal changes |
| `src/recorder.rs` | `recorder/media_recorder.rs` | Keep as-is |
| `src/storage.rs` | `recorder/storage.rs` | Add test metadata fields |
| `src/sensors.rs` | `recorder/sensors.rs` | Keep as-is |
| `src/types.rs` | `recorder/types.rs` | Keep recording types only |
| `src/ui.rs` | `recorder/ui.rs` | Adapt for new panel layout |
| `src/utils.rs` | `recorder/utils.rs` | Keep as-is |
| `js/indexed_db.js` | `server/static/lib/recorder/indexed_db.js` | Add test metadata |
| `js/media_recorder.js` | `server/static/lib/recorder/media_recorder.js` | Keep as-is |

## UI Design

### Recording Panel Layout

Add new collapsible section to `nettest.html` after the info section:

```
┌─────────────────────────────────────────┐
│ 📹 Recording                          ▼ │
├─────────────────────────────────────────┤
│ Mode: ○ Camera  ○ Screen  ● Combined   │
│                                         │
│ Camera PiP Controls:                    │
│   Size: [====○====] 25%                │
│   Position: [TL][TR][BL][BR]           │
│                                         │
│ Chart Overlay Controls:                 │
│   ☑ Include Charts                     │
│   Chart: [Metrics Chart ▾]             │
│   Size: [====○====] 20%                │
│   Position: [TL][TR][BL][BR]           │
│                                         │
│ ☑ Show Sensors Overlay                 │
│                                         │
│ [ Start Recording ]                     │
│                                         │
│ Saved Recordings:                       │
│ ┌───────────────────────────────────┐  │
│ │ 2026-02-04 14:23:45 | 1:23       │  │
│ │ [Download] [Download JSON] [Del]  │  │
│ └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

### Recording Modes

Three modes (same as camera app):

1. **Camera Only** - Webcam video with audio
2. **Screen Only** - Screen/window capture with audio + chart overlay
3. **Combined** - Screen + camera PiP + chart overlay + audio

### Controls

**Camera PiP Controls** (shown when Combined mode selected):
- Size slider: 10-40% of video width
- Position: 4-corner selector (TL/TR/BL/BR)
- Border/shadow: Rendered automatically

**Chart Overlay Controls**:
- Include charts: Checkbox (default: checked)
- Which chart: Dropdown
  - "Metrics Chart" (delay/throughput/jitter/loss)
  - "Probe Stats Chart"
  - "Both Charts" (stacked)
- Size: 10-30% of video width
- Position: 4-corner selector (default: bottom-right)

**Smart Positioning**: If camera PiP and chart overlay select same corner, auto-stack vertically to avoid overlap.

**Sensor Overlay Toggle**:
- Checkbox: "Show Sensors Overlay" (default: checked)
- Sensors still collected when hidden, just not rendered

### Status Indicator

Badge next to "Recording" header:
- 🔴 "Recording: 0:23" (pulsing red) - while recording
- ⚪ "Ready" - idle
- 💾 "Saving..." - finalizing to IndexedDB

## Canvas Compositing

### Rendering Pipeline

The camera app's existing `canvas_renderer.rs` already handles 30 FPS compositing. We extend it to add chart overlay.

**Compositing Layers** (bottom to top):

1. **Base video layer**: Camera/screen feed
2. **PiP layer** (combined mode only): Camera overlay with shadow/border
3. **Chart layer** (new): Latency graphs from Chart.js canvas
4. **Sensor overlay layer**: GPS/orientation/acceleration panel
5. **Compass layer**: Direction indicator

### Chart Integration

Chart.js already renders to `<canvas id="metrics-chart">`. We capture these pixels:

```rust
// In canvas_renderer.rs, after sensor overlay rendering
pub fn render_chart_overlay(
    &self,
    chart_element_id: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsValue> {
    let document = web_sys::window()
        .unwrap()
        .document()
        .unwrap();

    let chart_canvas: web_sys::HtmlCanvasElement =
        document.get_element_by_id(chart_element_id)?
        .dyn_into()?;

    self.ctx.draw_image_with_html_canvas_element_and_dw_and_dh(
        &chart_canvas, x, y, width, height
    )?;

    Ok(())
}
```

### Rendering Loop

Existing camera app pattern (30 FPS):

```rust
// Animation frame callback
fn render_frame() {
    // 1. Draw base video
    ctx.draw_image(video_element, ...);

    // 2. Draw PiP if combined mode
    if combined_mode {
        ctx.draw_image(pip_video, ...);
        // shadow/border effects
    }

    // 3. Draw chart overlay (NEW)
    if chart_enabled {
        render_chart_overlay("metrics-chart", chart_x, chart_y, w, h);
    }

    // 4. Draw sensor overlay
    render_sensor_overlay(gps, orientation, acceleration);

    // 5. Draw compass
    render_compass(direction);

    requestAnimationFrame(render_frame);
}
```

## Sensor Integration

### Sensor Data Collection

Same implementation as camera app:

**Sensors tracked:**
- GPS: Location, accuracy, altitude
- Orientation: Alpha/beta/gamma (device orientation)
- Acceleration: X/Y/Z movement
- Magnetometer: Compass heading (from orientation.alpha when absolute=true)

**Use case for network testing:**
- Correlate network performance with location (mobile testing)
- Understand device positioning during tests
- Detect if device is stationary vs. moving
- Context for outdoor/mobile testing scenarios

### Permission Flow

1. User clicks "Start Recording"
2. WASM calls `request_sensor_permissions()` helper
3. JavaScript prompts for iOS permissions (if needed)
4. Immediately attaches event listeners (same synchronous task - iOS requirement)
5. Starts GPS tracking
6. Begins recording with sensor collection active

### Global State

Same pattern as camera app:

```rust
// In recorder/mod.rs
use once_cell::sync::Lazy;
use std::sync::Mutex;

static SENSOR_MANAGER: Lazy<Mutex<Option<sensors::SensorManager>>> =
    Lazy::new(|| Mutex::new(None));
```

JavaScript sensor events call WASM exports that update global state:
- `on_gps_update()`
- `on_orientation()`
- `on_motion()`
- `on_magnetometer()`

Canvas renderer reads from global `SENSOR_MANAGER` each frame.

## Data Storage

### IndexedDB Schema

Database: `"NetpokeRecordingsDB"` (separate from camera app)
Object Store: `"recordings"`

**Recording Entry:**
```javascript
{
    id: <auto-increment>,
    timestamp: "2026-02-04T14:23:45.123Z",
    duration: 83.5,  // seconds
    frameCount: 2505,
    mimeType: "video/webm",
    sourceType: "camera" | "screen" | "combined",

    videoBlob: <Blob>,

    motionData: [
        {
            timestamp: "2026-02-04T14:23:45.456Z",
            gps: {
                latitude: 37.7749,
                longitude: -122.4194,
                accuracy: 10.5,
                altitude: 52.3,
                altitudeAccuracy: 5.0,
                heading: 90.0,
                speed: 0.0
            },
            orientation: {
                alpha: 180.5,
                beta: 10.2,
                gamma: 5.3,
                absolute: true
            },
            acceleration: {
                x: 0.1,
                y: -9.8,
                z: 0.2
            },
            magnetometer: {
                alpha: 180.5,
                beta: 0.0,
                gamma: 0.0,
                absolute: true
            }
        },
        // ... more points
    ],

    // New netpoke-specific fields
    chartIncluded: true,
    chartType: "metrics" | "probe-stats" | "both",
    testMetadata: {
        ipv4Active: true,
        ipv6Active: false,
        testStartTime: "2026-02-04T14:23:30.000Z",
        testEndTime: "2026-02-04T14:25:13.500Z"
    }
}
```

### Storage Operations

JavaScript API (called from WASM):
- `saveRecording(videoBlob, motionData, metadata)` - Save to IndexedDB
- `listRecordings()` - Get all recordings (for UI list)
- `getRecording(id)` - Get specific recording
- `deleteRecording(id)` - Delete recording
- `downloadVideo(id, filename)` - Trigger .webm download
- `downloadMotionData(id, filename)` - Trigger .json download

## Dependencies

### Cargo.toml Updates

Add to `netpoke/client/Cargo.toml`:

```toml
[dependencies]
# ... existing dependencies ...

# For global sensor manager
once_cell = "1.19"

# Additional web-sys features for recording
web-sys = { version = "0.3", features = [
    # ... existing features ...

    # Media APIs
    "MediaDevices",
    "MediaStream",
    "MediaStreamTrack",
    "MediaStreamConstraints",
    "MediaTrackConstraints",
    "MediaRecorder",
    "MediaRecorderOptions",
    "BlobEvent",

    # Canvas/Video
    "CanvasRenderingContext2d",
    "HtmlCanvasElement",
    "HtmlVideoElement",
    "HtmlInputElement",
    "VideoTrack",
    "AudioTrack",

    # Sensors
    "Geolocation",
    "GeolocationPosition",
    "GeolocationCoordinates",
    "GeolocationPositionError",

    # IndexedDB
    "IdbFactory",
    "IdbDatabase",
    "IdbObjectStore",
    "IdbTransaction",
    "IdbRequest",
    "IdbCursorWithValue",
    "IdbTransactionMode",
    "IdbRequestReadyState",
] }
```

### JavaScript Dependencies

No new npm packages needed:
- Chart.js: Already loaded for existing graphs
- IndexedDB/MediaRecorder wrappers: Custom JS from camera app

## HTML Integration

### Update nettest.html

**1. Add recording section** (after info section):

```html
<div class="recording-section">
    <div class="recording-header" onclick="toggleRecording()">
        <h2>
            <span class="recording-icon">📹</span>
            Recording
            <span id="recording-status" class="status-badge">Ready</span>
        </h2>
        <button class="recording-toggle" id="recording-toggle">▼</button>
    </div>
    <div class="recording-content" id="recording-content" style="display:none">
        <!-- Recording controls rendered by WASM -->
    </div>
</div>
```

**2. Add hidden compositing elements:**

```html
<div id="hiddenVideos" style="position:absolute;opacity:0;pointer-events:none;width:1px;height:1px;">
    <video id="cameraVideo" autoplay muted playsinline></video>
    <video id="screenVideo" autoplay muted playsinline></video>
    <canvas id="recordingCanvas"></canvas>
</div>
```

**3. Add script imports:**

```html
<!-- Recorder JavaScript -->
<script src="/static/lib/recorder/indexed_db.js"></script>
<script src="/static/lib/recorder/media_recorder.js"></script>

<script type="module">
    import init, { init_netpoke, init_recorder } from '/static/pkg/netpoke_client.js';

    async function run() {
        await init();
        init_netpoke();     // Existing
        init_recorder();    // New
    }
    run();
</script>
```

**4. Cache busting:**

Add version constant and append to imports:
```javascript
const APP_VERSION = 1;
const wasmUrl = `/static/pkg/netpoke_client_bg.wasm?v=${APP_VERSION}`;
```

## Error Handling

### Permission Denied

If user denies permissions:
- Show error in recording panel: "⚠️ Permissions required. Please allow access and try again."
- Disable recording buttons
- Provide "Retry Permissions" button

### Browser Compatibility

Check on page load:
```javascript
const support = {
    mediaRecorder: typeof MediaRecorder !== 'undefined',
    displayMedia: !!navigator.mediaDevices?.getDisplayMedia,
    userMedia: !!navigator.mediaDevices?.getUserMedia
};
```

If features missing:
- Show compatibility warning
- Disable unsupported modes
- Display fallback message

### Storage Quota

If IndexedDB quota exceeded:
- Alert: "Storage full. Please delete old recordings."
- Keep recording in memory temporarily
- Offer immediate download
- Show storage usage indicator

### MediaRecorder Failures

- Codec not supported → Try fallback (VP8 ↔ H.264)
- Stream ended → Save partial recording with warning
- Browser crash → Recording lost (unavoidable)

### Mobile Considerations

- iOS Safari: Limited codec support (handled in media_recorder.js)
- Battery warning: If recording >5 minutes on mobile
- Background tab: Recording stops (browser limitation, warn user)

## Testing Strategy

### Desktop Testing (Chrome/Firefox/Safari)

- [ ] Camera-only mode records and saves
- [ ] Screen-only mode with chart overlay
- [ ] Combined mode: screen + camera PiP + chart
- [ ] Sensor overlay toggles correctly
- [ ] Chart position/size controls work
- [ ] Save to IndexedDB works
- [ ] Download video (.webm) works
- [ ] Download motion data (.json) works
- [ ] Delete recording works
- [ ] Multiple recordings supported
- [ ] Charts continue updating during recording

### Mobile Testing (iOS/Android)

- [ ] Permission prompts appear
- [ ] GPS data collected
- [ ] Orientation/acceleration data collected
- [ ] Compass renders
- [ ] No network test performance degradation
- [ ] Battery usage acceptable (1-2 min recordings)

### Integration Testing

- [ ] Start recording mid-test → chart appears in video
- [ ] Recording across multiple test runs
- [ ] Stop test while recording → recording continues
- [ ] Network test accuracy unchanged while recording

### Performance Benchmarks

Monitor during recording:
- Canvas render time: <5ms per frame (30 FPS)
- Network test latency: No >10% increase
- Memory usage: <100 MB for 5-minute recording

## Deployment

### Build Process

```bash
# 1. Build WASM client with recorder
cd netpoke/client
./build.sh  # wasm-pack build --target web --out-dir pkg --release

# 2. Copy to server static directory
# (handled by existing deployment scripts)

# 3. Update APP_VERSION in nettest.html

# 4. Deploy server
```

### Staged Rollout

**Phase 1: Basic Integration**
- Merge camera modules into netpoke-client
- Add recording panel UI (collapsed by default)
- Camera-only and screen-only modes
- No chart overlay (simplest first)

**Phase 2: Chart Compositing**
- Add chart overlay controls
- Implement chart canvas capture
- Test positioning/sizing

**Phase 3: Polish**
- Combined mode with all layers
- Mobile testing
- Error handling refinement

### Backwards Compatibility

- Recording feature opt-in (panel collapsed by default)
- No changes to existing netpoke functionality
- Network testing works without recording permissions
- No breaking changes

## Documentation

Add to `netpoke/docs/`:
- `RECORDING_FEATURE.md` - User guide
- Update main README with feature mention

## Open Questions

None - design complete and approved.

## References

- Camera app: `/camera/`
- Camera STATE.md: `/camera/STATE.md`
- Netpoke client: `/netpoke/client/`
- Chart.js integration: `/netpoke/server/static/nettest.html` (lines 803-1924)
