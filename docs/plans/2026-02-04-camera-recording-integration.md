# Camera Recording Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Integrate camera/screen recording functionality from the standalone camera WASM project into netpoke client, enabling users to record network tests with video, sensor overlays, and composited latency graphs.

**Architecture:** Merge camera's Rust WASM modules into netpoke-client as a new `recorder/` submodule. Extend existing canvas rendering pipeline to composite Chart.js graphs alongside sensor overlays. Store recordings in IndexedDB with test metadata.

**Tech Stack:** Rust + WASM (wasm-bindgen, web-sys), Chart.js (existing), IndexedDB, MediaRecorder API, WebRTC (existing), Geolocation/DeviceOrientation APIs

**Design Document:** `docs/plans/2026-02-04-camera-recording-integration-design.md`

---

## Phase 1: Project Structure Setup

### Task 1: Create Recorder Module Structure

**Files:**
- Create: `client/src/recorder/mod.rs`
- Modify: `client/src/lib.rs`

**Step 1: Create recorder module directory and mod.rs**

```bash
mkdir -p client/src/recorder
```

Create `client/src/recorder/mod.rs`:
```rust
//! Recording subsystem for capturing video with sensor overlays and chart compositing

pub mod types;
pub mod utils;
pub mod sensors;
pub mod canvas_renderer;
pub mod media_streams;
pub mod media_recorder;
pub mod storage;
pub mod state;
pub mod ui;

// Re-export main entry points
pub use state::RecorderState;
pub use ui::init_recorder_panel;
```

**Step 2: Add recorder module to lib.rs**

Modify `client/src/lib.rs` at the top (after existing mod declarations):
```rust
mod recorder;
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success (empty modules will be added next)

**Step 4: Commit**

```bash
git add client/src/recorder/mod.rs client/src/lib.rs
git commit -m "feat(recorder): add recorder module structure

Set up module hierarchy for camera recording integration.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 2: Copy Type Definitions

**Files:**
- Create: `client/src/recorder/types.rs`
- Reference: `../camera/src/types.rs`

**Step 1: Copy types.rs from camera**

```bash
cp ../camera/src/types.rs client/src/recorder/types.rs
```

**Step 2: Review and clean up**

Open `client/src/recorder/types.rs` and verify it contains:
- `SourceType` enum (Camera, Screen, Combined)
- `PipPosition` enum (TopLeft, TopRight, BottomLeft, BottomRight)
- `GpsData` struct
- `OrientationData` struct
- `AccelerationData` struct
- `MotionDataPoint` struct
- `RecordingMetadata` struct

All types should have `#[derive(Clone, Debug, Serialize, Deserialize)]` and `#[serde(default)]` where appropriate.

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add client/src/recorder/types.rs
git commit -m "feat(recorder): add type definitions

Copy recording types from camera project: SourceType, PipPosition,
sensor data structures, and metadata types.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 3: Copy Utility Functions

**Files:**
- Create: `client/src/recorder/utils.rs`
- Reference: `../camera/src/utils.rs`

**Step 1: Copy utils.rs from camera**

```bash
cp ../camera/src/utils.rs client/src/recorder/utils.rs
```

**Step 2: Verify it contains**

- `log()` - Logging helper
- `format_timestamp()` - UTC timestamp formatting
- `format_duration()` - Human-readable duration

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add client/src/recorder/utils.rs
git commit -m "feat(recorder): add utility functions

Copy timestamp formatting and logging utilities from camera project.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 4: Add Dependencies to Cargo.toml

**Files:**
- Modify: `client/Cargo.toml`

**Step 1: Add once_cell dependency**

Add to `[dependencies]` section:
```toml
once_cell = "1.19"
```

**Step 2: Add web-sys features for recording**

Add these features to the existing `web-sys` dependency features list:
```toml
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

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Downloads new dependencies, compiles successfully

**Step 4: Commit**

```bash
git add client/Cargo.toml
git commit -m "feat(recorder): add dependencies for recording

Add once_cell for global state and web-sys features for MediaRecorder,
Canvas, Sensors, and IndexedDB APIs.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 2: Sensor Tracking

### Task 5: Implement Sensor Manager

**Files:**
- Create: `client/src/recorder/sensors.rs`
- Reference: `../camera/src/sensors.rs`

**Step 1: Copy sensors.rs from camera**

```bash
cp ../camera/src/sensors.rs client/src/recorder/sensors.rs
```

**Step 2: Verify SensorManager contains**

- `new(start_time, camera_facing)` - Constructor
- `record_gps()` - Record GPS data point
- `record_orientation()` - Record orientation data
- `record_motion()` - Record acceleration data
- `record_magnetometer()` - Record magnetometer data
- `get_motion_data()` - Retrieve all collected data
- `get_current_gps()` - Get latest GPS
- `get_current_orientation()` - Get latest orientation
- `set_overlay_enabled()` - Toggle overlay rendering

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add client/src/recorder/sensors.rs
git commit -m "feat(recorder): add sensor manager

Implement SensorManager for collecting GPS, orientation, acceleration,
and magnetometer data during recording.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 6: Add Global Sensor Manager and WASM Exports

**Files:**
- Modify: `client/src/lib.rs`

**Step 1: Add global SENSOR_MANAGER**

Add at top of `client/src/lib.rs` after imports:
```rust
use once_cell::sync::Lazy;
use std::sync::Mutex;

static SENSOR_MANAGER: Lazy<Mutex<Option<recorder::sensors::SensorManager>>> =
    Lazy::new(|| Mutex::new(None));
```

**Step 2: Add WASM exports for sensor callbacks**

Add these functions to `client/src/lib.rs`:
```rust
#[wasm_bindgen]
pub fn on_gps_update(
    latitude: f64,
    longitude: f64,
    accuracy: f64,
    altitude: Option<f64>,
    altitude_accuracy: Option<f64>,
    heading: Option<f64>,
    speed: Option<f64>,
) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            mgr.record_gps(
                latitude,
                longitude,
                accuracy,
                altitude,
                altitude_accuracy,
                heading,
                speed,
            );
        }
    }
}

#[wasm_bindgen]
pub fn on_orientation(alpha: f64, beta: f64, gamma: f64, absolute: bool) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            mgr.record_orientation(alpha, beta, gamma, absolute);
        }
    }
}

#[wasm_bindgen]
pub fn on_motion(x: f64, y: f64, z: f64) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            mgr.record_motion(x, y, z);
        }
    }
}

#[wasm_bindgen]
pub fn on_magnetometer(alpha: f64, beta: f64, gamma: f64, absolute: bool) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            mgr.record_magnetometer(alpha, beta, gamma, absolute);
        }
    }
}
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add client/src/lib.rs
git commit -m "feat(recorder): add sensor callbacks

Add global SENSOR_MANAGER and WASM exports for JavaScript sensor
event callbacks (GPS, orientation, motion, magnetometer).

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 3: Media Capture

### Task 7: Implement Media Streams

**Files:**
- Create: `client/src/recorder/media_streams.rs`
- Reference: `../camera/src/media_streams.rs`

**Step 1: Copy media_streams.rs from camera**

```bash
cp ../camera/src/media_streams.rs client/src/recorder/media_streams.rs
```

**Step 2: Verify it contains**

- `get_camera_stream()` - Request camera access
- `get_screen_stream()` - Request screen sharing
- Helper functions for MediaStreamConstraints

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add client/src/recorder/media_streams.rs
git commit -m "feat(recorder): add media stream capture

Implement camera and screen capture using getUserMedia and
getDisplayMedia APIs.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 8: Implement Media Recorder Wrapper

**Files:**
- Create: `client/src/recorder/media_recorder.rs`
- Reference: `../camera/src/recorder.rs`

**Step 1: Copy recorder.rs as media_recorder.rs**

```bash
cp ../camera/src/recorder.rs client/src/recorder/media_recorder.rs
```

**Step 2: Verify it contains**

- `Recorder` struct
- `new()` - Create recorder with canvas stream
- `start()` - Begin recording
- `stop()` - Stop recording and get blob
- Event handlers for dataavailable

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add client/src/recorder/media_recorder.rs
git commit -m "feat(recorder): add media recorder wrapper

Implement MediaRecorder wrapper for capturing canvas stream to video
blob with codec selection.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 4: Canvas Rendering

### Task 9: Copy Canvas Renderer Base

**Files:**
- Create: `client/src/recorder/canvas_renderer.rs`
- Reference: `../camera/src/canvas_renderer.rs`

**Step 1: Copy canvas_renderer.rs from camera**

```bash
cp ../camera/src/canvas_renderer.rs client/src/recorder/canvas_renderer.rs
```

**Step 2: Verify it contains**

- `CanvasRenderer` struct
- `new()` - Initialize with canvas
- `render_camera()` - Render camera feed
- `render_screen()` - Render screen feed
- `render_combined()` - Render screen + PiP camera
- `render_sensor_overlay()` - Render GPS/orientation/acceleration panel
- `render_compass()` - Render compass indicator

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add client/src/recorder/canvas_renderer.rs
git commit -m "feat(recorder): add canvas renderer base

Copy canvas compositing system from camera project with sensor overlay
and PiP rendering.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 10: Add Chart Overlay Rendering

**Files:**
- Modify: `client/src/recorder/canvas_renderer.rs`

**Step 1: Add render_chart_overlay method**

Add to `CanvasRenderer` impl block:
```rust
/// Composite Chart.js canvas into recording
pub fn render_chart_overlay(
    &self,
    chart_element_id: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), JsValue> {
    let document = web_sys::window()
        .ok_or("No window")?
        .document()
        .ok_or("No document")?;

    let chart_canvas: web_sys::HtmlCanvasElement = document
        .get_element_by_id(chart_element_id)
        .ok_or("Chart canvas not found")?
        .dyn_into()
        .map_err(|_| "Element is not a canvas")?;

    self.ctx
        .draw_image_with_html_canvas_element_and_dw_and_dh(
            &chart_canvas,
            x,
            y,
            width,
            height,
        )?;

    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add client/src/recorder/canvas_renderer.rs
git commit -m "feat(recorder): add chart overlay rendering

Add method to composite Chart.js canvas into recording at specified
position and size.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 5: Storage

### Task 11: Implement IndexedDB Storage

**Files:**
- Create: `client/src/recorder/storage.rs`
- Reference: `../camera/src/storage.rs`

**Step 1: Copy storage.rs from camera**

```bash
cp ../camera/src/storage.rs client/src/recorder/storage.rs
```

**Step 2: Add test metadata fields**

Modify the `save_recording` function to accept additional test metadata:
```rust
pub async fn save_recording(
    blob: web_sys::Blob,
    duration: f64,
    frame_count: u32,
    source_type: &str,
    motion_data: Vec<crate::recorder::types::MotionDataPoint>,
    chart_included: bool,
    chart_type: Option<String>,
    test_metadata: Option<serde_json::Value>,
) -> Result<(), JsValue> {
    // ... existing implementation with added fields
}
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add client/src/recorder/storage.rs
git commit -m "feat(recorder): add IndexedDB storage with test metadata

Implement recording storage with test metadata fields for network test
correlation.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 6: Recording State Management

### Task 12: Implement Recorder State

**Files:**
- Create: `client/src/recorder/state.rs`
- Reference: `../camera/src/app.rs`

**Step 1: Create RecorderState struct**

Create `client/src/recorder/state.rs`:
```rust
use crate::recorder::{
    canvas_renderer::CanvasRenderer,
    media_recorder::Recorder,
    sensors::SensorManager,
    types::{SourceType, PipPosition},
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::MediaStream;

pub struct RecorderState {
    pub source_type: SourceType,
    pub pip_position: PipPosition,
    pub pip_size: f64,
    pub chart_enabled: bool,
    pub chart_type: String,
    pub chart_position: PipPosition,
    pub chart_size: f64,
    pub recording: bool,
    pub start_time: f64,
    pub frame_count: u32,

    camera_stream: Option<MediaStream>,
    screen_stream: Option<MediaStream>,
    renderer: Option<CanvasRenderer>,
    recorder: Option<Recorder>,
    animation_frame_id: Option<i32>,
}

impl RecorderState {
    pub fn new() -> Self {
        Self {
            source_type: SourceType::Combined,
            pip_position: PipPosition::TopLeft,
            pip_size: 0.25,
            chart_enabled: true,
            chart_type: "metrics-chart".to_string(),
            chart_position: PipPosition::BottomRight,
            chart_size: 0.20,
            recording: false,
            start_time: 0.0,
            frame_count: 0,
            camera_stream: None,
            screen_stream: None,
            renderer: None,
            recorder: None,
            animation_frame_id: None,
        }
    }

    pub async fn start_recording(&mut self) -> Result<(), JsValue> {
        // Implementation in next task
        todo!()
    }

    pub async fn stop_recording(&mut self) -> Result<(), JsValue> {
        // Implementation in next task
        todo!()
    }
}
```

**Step 2: Verify compilation**

Run: `cd client && cargo check`
Expected: Warning about unused fields (expected), no errors

**Step 3: Commit**

```bash
git add client/src/recorder/state.rs
git commit -m "feat(recorder): add recorder state structure

Define RecorderState struct to manage recording session lifecycle and
configuration.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 13: Implement Start Recording

**Files:**
- Modify: `client/src/recorder/state.rs`

**Step 1: Replace start_recording implementation**

Replace the `todo!()` in `start_recording`:
```rust
pub async fn start_recording(&mut self) -> Result<(), JsValue> {
    use crate::recorder::media_streams::{get_camera_stream, get_screen_stream};
    use crate::recorder::utils::log;

    log("[Recorder] Starting recording");

    // Get media streams based on source type
    match self.source_type {
        SourceType::Camera => {
            self.camera_stream = Some(get_camera_stream(false).await?);
        }
        SourceType::Screen => {
            self.screen_stream = Some(get_screen_stream().await?);
        }
        SourceType::Combined => {
            self.camera_stream = Some(get_camera_stream(true).await?);
            self.screen_stream = Some(get_screen_stream().await?);
        }
    }

    // Initialize canvas renderer
    let document = web_sys::window()
        .ok_or("No window")?
        .document()
        .ok_or("No document")?;

    let canvas: web_sys::HtmlCanvasElement = document
        .get_element_by_id("recordingCanvas")
        .ok_or("recordingCanvas not found")?
        .dyn_into()?;

    self.renderer = Some(CanvasRenderer::new(canvas.clone())?);

    // Start MediaRecorder with canvas stream
    let canvas_stream = canvas
        .capture_stream()
        .map_err(|_| "Failed to capture canvas stream")?;

    self.recorder = Some(Recorder::new(canvas_stream)?);
    if let Some(recorder) = &self.recorder {
        recorder.start()?;
    }

    self.recording = true;
    self.start_time = js_sys::Date::now();
    self.frame_count = 0;

    // Start render loop
    self.start_render_loop()?;

    log("[Recorder] Recording started");
    Ok(())
}

fn start_render_loop(&mut self) -> Result<(), JsValue> {
    // This will be implemented with render_frame callback
    // For now, placeholder
    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add client/src/recorder/state.rs
git commit -m "feat(recorder): implement start recording

Acquire media streams, initialize canvas renderer and MediaRecorder,
begin recording session.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 14: Implement Stop Recording

**Files:**
- Modify: `client/src/recorder/state.rs`

**Step 1: Replace stop_recording implementation**

Replace the `todo!()` in `stop_recording`:
```rust
pub async fn stop_recording(&mut self) -> Result<(), JsValue> {
    use crate::recorder::{storage, utils::log};

    log("[Recorder] Stopping recording");

    // Stop render loop
    if let Some(id) = self.animation_frame_id {
        web_sys::window()
            .ok_or("No window")?
            .cancel_animation_frame(id)?;
        self.animation_frame_id = None;
    }

    // Stop MediaRecorder and get blob
    let blob = if let Some(recorder) = &self.recorder {
        recorder.stop().await?
    } else {
        return Err("No recorder".into());
    };

    // Get motion data from global SENSOR_MANAGER
    let motion_data = if let Ok(manager_guard) = crate::SENSOR_MANAGER.lock() {
        if let Some(ref mgr) = *manager_guard {
            mgr.get_motion_data().clone()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Calculate duration
    let duration = (js_sys::Date::now() - self.start_time) / 1000.0;

    // Get test metadata (placeholder for now)
    let test_metadata = None;

    // Save to IndexedDB
    storage::save_recording(
        blob,
        duration,
        self.frame_count,
        &format!("{:?}", self.source_type),
        motion_data,
        self.chart_enabled,
        if self.chart_enabled {
            Some(self.chart_type.clone())
        } else {
            None
        },
        test_metadata,
    )
    .await?;

    // Cleanup
    self.camera_stream = None;
    self.screen_stream = None;
    self.recorder = None;
    self.renderer = None;
    self.recording = false;

    log("[Recorder] Recording saved");
    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cd client && cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add client/src/recorder/state.rs
git commit -m "feat(recorder): implement stop recording

Stop render loop, finalize MediaRecorder, collect sensor data, save to
IndexedDB with metadata.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 7: UI Integration

### Task 15: Implement UI Module

**Files:**
- Create: `client/src/recorder/ui.rs`

**Step 1: Create UI initialization**

Create `client/src/recorder/ui.rs`:
```rust
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use std::cell::RefCell;
use std::rc::Rc;

use crate::recorder::state::RecorderState;

thread_local! {
    static RECORDER_STATE: Rc<RefCell<RecorderState>> =
        Rc::new(RefCell::new(RecorderState::new()));
}

#[wasm_bindgen]
pub fn init_recorder_panel() {
    let document = match web_sys::window()
        .and_then(|w| w.document())
    {
        Some(d) => d,
        None => return,
    };

    // Set up recording panel controls
    setup_mode_selection(&document);
    setup_pip_controls(&document);
    setup_chart_controls(&document);
    setup_recording_buttons(&document);

    crate::recorder::utils::log("[Recorder] Panel initialized");
}

fn setup_mode_selection(document: &web_sys::Document) {
    // Implementation placeholder
}

fn setup_pip_controls(document: &web_sys::Document) {
    // Implementation placeholder
}

fn setup_chart_controls(document: &web_sys::Document) {
    // Implementation placeholder
}

fn setup_recording_buttons(document: &web_sys::Document) {
    // Attach event listeners to start/stop buttons
    // Implementation placeholder
}
```

**Step 2: Export init_recorder from lib.rs**

Add to `client/src/lib.rs`:
```rust
#[wasm_bindgen]
pub fn init_recorder() {
    recorder::ui::init_recorder_panel();
}
```

**Step 3: Verify compilation**

Run: `cd client && cargo check`
Expected: Success with warnings about unused functions

**Step 4: Commit**

```bash
git add client/src/recorder/ui.rs client/src/lib.rs
git commit -m "feat(recorder): add UI initialization skeleton

Set up recorder panel initialization and export from WASM.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 16: Copy JavaScript Helpers

**Files:**
- Create: `server/static/lib/recorder/indexed_db.js`
- Create: `server/static/lib/recorder/media_recorder.js`
- Reference: `../camera/js/`

**Step 1: Create recorder lib directory**

```bash
mkdir -p server/static/lib/recorder
```

**Step 2: Copy JavaScript files**

```bash
cp ../camera/js/indexed_db.js server/static/lib/recorder/
cp ../camera/js/media_recorder.js server/static/lib/recorder/
```

**Step 3: Verify files copied**

Run: `ls -la server/static/lib/recorder/`
Expected: Both .js files present

**Step 4: Commit**

```bash
git add server/static/lib/recorder/
git commit -m "feat(recorder): add JavaScript helper libraries

Copy IndexedDB and MediaRecorder wrapper libraries from camera project.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 8: HTML Integration

### Task 17: Add Recording Panel HTML

**Files:**
- Modify: `server/static/nettest.html`

**Step 1: Add recording section after info section**

Find the closing `</div>` of the info-section and add after it:

```html
<!-- Recording Section -->
<div class="recording-section">
    <div class="recording-header" onclick="toggleRecordingPanel()">
        <h2>
            <span class="recording-icon">📹</span>
            Recording
            <span id="recording-status" class="status-badge">Ready</span>
        </h2>
        <button class="recording-toggle" id="recording-toggle">▼</button>
    </div>
    <div class="recording-content" id="recording-content" style="display:none">

        <!-- Mode Selection -->
        <div class="recording-mode-group">
            <h3>Recording Mode</h3>
            <label><input type="radio" name="recordMode" value="camera"> Camera Only</label>
            <label><input type="radio" name="recordMode" value="screen"> Screen Only</label>
            <label><input type="radio" name="recordMode" value="combined" checked> Combined (PiP)</label>
        </div>

        <!-- PiP Controls -->
        <div id="pip-controls" class="control-group">
            <h3>Camera Position</h3>
            <label>
                Size: <input type="range" id="pip-size" min="10" max="40" value="25">
                <span id="pip-size-value">25%</span>
            </label>
            <div class="position-selector">
                <button data-position="topleft">TL</button>
                <button data-position="topright">TR</button>
                <button data-position="bottomleft">BL</button>
                <button data-position="bottomright" class="selected">BR</button>
            </div>
        </div>

        <!-- Chart Controls -->
        <div id="chart-controls" class="control-group">
            <h3>Chart Overlay</h3>
            <label><input type="checkbox" id="chart-enabled" checked> Include Charts</label>
            <label>
                Chart:
                <select id="chart-type">
                    <option value="metrics-chart">Metrics Chart</option>
                    <option value="probe-stats-chart">Probe Stats</option>
                    <option value="both">Both Charts</option>
                </select>
            </label>
            <label>
                Size: <input type="range" id="chart-size" min="10" max="30" value="20">
                <span id="chart-size-value">20%</span>
            </label>
            <div class="position-selector">
                <button data-position="topleft">TL</button>
                <button data-position="topright">TR</button>
                <button data-position="bottomleft">BL</button>
                <button data-position="bottomright" class="selected">BR</button>
            </div>
        </div>

        <!-- Sensor Toggle -->
        <div class="control-group">
            <label><input type="checkbox" id="showSensorsOverlay" checked> Show Sensors</label>
        </div>

        <!-- Recording Buttons -->
        <button id="start-recording" class="btn-primary">Start Recording</button>
        <button id="stop-recording" class="btn-danger" style="display:none">Stop Recording</button>

        <!-- Saved Recordings List -->
        <div id="recordings-list" class="recordings-list">
            <h3>Saved Recordings</h3>
            <div id="recordings-container"></div>
        </div>

    </div>
</div>
```

**Step 2: Add hidden canvas/video elements**

Add before closing `</body>`:

```html
<!-- Hidden elements for recording -->
<div id="hiddenVideos" style="position:absolute;opacity:0;pointer-events:none;width:1px;height:1px;">
    <video id="cameraVideo" autoplay muted playsinline></video>
    <video id="screenVideo" autoplay muted playsinline></video>
    <canvas id="recordingCanvas"></canvas>
</div>
```

**Step 3: Verify HTML is valid**

Open nettest.html in browser and check console for errors
Expected: No HTML parse errors

**Step 4: Commit**

```bash
git add server/static/nettest.html
git commit -m "feat(recorder): add recording panel HTML

Add collapsible recording section with mode selection, PiP controls,
chart controls, and recordings list.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 18: Add Recording Panel CSS

**Files:**
- Modify: `server/static/nettest.html` (in `<style>` section)

**Step 1: Add recording section styles**

Add to the `<style>` section:

```css
/* Recording Section */
.recording-section {
    background: white;
    border-radius: 12px;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
    margin-bottom: 24px;
    overflow: hidden;
}

.recording-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    cursor: pointer;
    user-select: none;
}

.recording-header h2 {
    font-size: 18px;
    font-weight: 600;
    color: #333;
    display: flex;
    align-items: center;
    gap: 8px;
}

.recording-icon {
    font-size: 20px;
}

.status-badge {
    font-size: 11px;
    padding: 4px 10px;
    border-radius: 6px;
    font-weight: 500;
    margin-left: 8px;
}

.status-badge.ready {
    background-color: rgba(76, 175, 80, 0.1);
    color: #4CAF50;
}

.status-badge.recording {
    background-color: rgba(244, 67, 54, 0.1);
    color: #F44336;
    animation: pulse 1.5s infinite;
}

@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
}

.status-badge.saving {
    background-color: rgba(33, 150, 243, 0.1);
    color: #2196F3;
}

.recording-toggle {
    background: none;
    border: none;
    font-size: 14px;
    color: #666;
    cursor: pointer;
    padding: 4px 8px;
    transition: transform 0.2s;
}

.recording-toggle.open {
    transform: rotate(180deg);
}

.recording-content {
    padding: 0 24px 24px 24px;
}

.recording-mode-group {
    margin-bottom: 20px;
}

.recording-mode-group h3 {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 8px;
    color: #555;
}

.recording-mode-group label {
    display: inline-block;
    margin-right: 16px;
    font-size: 14px;
}

.control-group {
    margin-bottom: 20px;
    padding: 12px;
    background: #f9f9f9;
    border-radius: 8px;
}

.control-group h3 {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 8px;
    color: #555;
}

.control-group label {
    display: block;
    margin-bottom: 8px;
    font-size: 14px;
}

.position-selector {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-top: 8px;
}

.position-selector button {
    padding: 8px;
    border: 2px solid #ddd;
    background: white;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 600;
    transition: all 0.2s;
}

.position-selector button:hover {
    border-color: #2196F3;
}

.position-selector button.selected {
    background: #2196F3;
    color: white;
    border-color: #2196F3;
}

.btn-primary {
    width: 100%;
    padding: 12px;
    background: #4CAF50;
    color: white;
    border: none;
    border-radius: 8px;
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
    margin-bottom: 8px;
}

.btn-primary:hover {
    background: #45a049;
}

.btn-danger {
    width: 100%;
    padding: 12px;
    background: #F44336;
    color: white;
    border: none;
    border-radius: 8px;
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
}

.btn-danger:hover {
    background: #da190b;
}

.recordings-list {
    margin-top: 24px;
    padding-top: 24px;
    border-top: 1px solid #eee;
}

.recordings-list h3 {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 12px;
    color: #555;
}

#recordings-container {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.recording-item {
    background: #f9f9f9;
    padding: 12px;
    border-radius: 8px;
    display: flex;
    justify-content: space-between;
    align-items: center;
}

.recording-item-info {
    flex: 1;
}

.recording-item-actions {
    display: flex;
    gap: 8px;
}

.recording-item-actions button {
    padding: 6px 12px;
    font-size: 12px;
    border: 1px solid #ddd;
    background: white;
    border-radius: 6px;
    cursor: pointer;
}

.recording-item-actions button:hover {
    background: #f0f0f0;
}
```

**Step 2: Verify styles applied**

Open nettest.html in browser and check recording section appearance
Expected: Styled recording panel visible (collapsed by default)

**Step 3: Commit**

```bash
git add server/static/nettest.html
git commit -m "feat(recorder): add recording panel CSS

Style recording panel with collapsible header, controls, and
recordings list.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 19: Add Recording Panel JavaScript

**Files:**
- Modify: `server/static/nettest.html` (in `<script>` section)

**Step 1: Add panel toggle function**

Add to script section:

```javascript
// Recording panel toggle
function toggleRecordingPanel() {
    const content = document.getElementById('recording-content');
    const toggle = document.getElementById('recording-toggle');

    if (content.style.display === 'none') {
        content.style.display = 'block';
        toggle.classList.add('open');
    } else {
        content.style.display = 'none';
        toggle.classList.remove('open');
    }
}
```

**Step 2: Add imports for recorder libraries**

Add before closing `</body>`:

```html
<!-- Recorder JavaScript -->
<script src="/static/lib/recorder/indexed_db.js"></script>
<script src="/static/lib/recorder/media_recorder.js"></script>
```

**Step 3: Add recorder initialization to module script**

Modify the existing module script to add:

```javascript
import init, { init_netpoke, init_recorder } from '/static/pkg/netpoke_client.js';

async function run() {
    await init();
    init_netpoke();      // Existing
    init_recorder();     // New
}
run();
```

**Step 4: Verify in browser**

Open nettest.html and check console for init_recorder call
Expected: No errors, panel toggles on click

**Step 5: Commit**

```bash
git add server/static/nettest.html
git commit -m "feat(recorder): add recording panel JavaScript

Add panel toggle, import recorder libraries, initialize recorder on
page load.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 9: Build and Integration Testing

### Task 20: Build WASM Client

**Files:**
- Build output: `client/pkg/`

**Step 1: Build release WASM**

```bash
cd client
./build.sh
```

Or manually:
```bash
cd client
wasm-pack build --target web --out-dir pkg --release
```

**Step 2: Verify build output**

Run: `ls -la client/pkg/`
Expected: `netpoke_client_bg.wasm`, `netpoke_client.js`, `package.json`

**Step 3: Check build size**

Run: `du -h client/pkg/netpoke_client_bg.wasm`
Expected: Should be reasonable size (few hundred KB)

**Step 4: Commit (if build config changed)**

Only if build.sh was modified:
```bash
git add client/build.sh
git commit -m "build(recorder): update build script

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

### Task 21: Manual Testing Checklist

**Testing Steps:**

**Test 1: Page loads without errors**
1. Start server: `cargo run --bin netpoke-server`
2. Open browser to nettest.html
3. Check browser console for errors
Expected: No JavaScript errors, recording panel visible and toggles

**Test 2: Recording panel UI responds**
1. Click recording panel header to expand
2. Change recording mode radio buttons
3. Adjust PiP size slider
4. Click position buttons
Expected: All controls respond, values update

**Test 3: Basic permissions flow**
1. Click "Start Recording"
2. Allow camera/screen permissions when prompted
Expected: Permission prompts appear (functionality not complete yet, just checking UI)

**Document results:**
Create test notes in a comment or separate file documenting what works and what needs completion.

---

## Phase 10: Implementation Completion

### Task 22: Implement Render Loop

**Files:**
- Modify: `client/src/recorder/state.rs`

**Step 1: Add render_frame method**

This requires closures and animation frame handling. Implementation will be added to properly composite all layers (camera/screen, PiP, charts, sensors, compass) at 30 FPS.

**Note:** This is a complex task requiring careful handling of Rust closures with wasm-bindgen. Consider implementing as a focused task using @superpowers:test-driven-development.

### Task 23: Connect UI Controls to State

**Files:**
- Modify: `client/src/recorder/ui.rs`

**Step 1: Implement event listeners**

Complete the placeholder functions to:
- Listen to mode selection changes
- Update RecorderState when controls change
- Enable/disable controls based on recording state
- Update status badge text

### Task 24: Test End-to-End Recording

**Testing:**
1. Start a network test
2. Start recording in combined mode
3. Verify canvas updates with all layers
4. Stop recording after 10 seconds
5. Check IndexedDB for saved recording
6. Download video and verify content
7. Download JSON and verify sensor data

### Task 25: Polish and Documentation

**Files:**
- Create: `docs/RECORDING_FEATURE.md`
- Modify: `README.md`

**Step 1: Write user documentation**

Create user guide explaining:
- How to use recording feature
- Recording modes and when to use each
- Chart overlay configuration
- Sensor data interpretation
- Downloading and managing recordings

**Step 2: Update README**

Add recording feature to main feature list with link to detailed docs.

---

## Notes

- **Phase 1-8** focus on structure and integration
- **Phase 9** validates the build and basic UI
- **Phase 10** completes functionality and adds polish
- Each task is small and testable
- Commit frequently with descriptive messages
- Use @superpowers:test-driven-development for complex Rust async/closure tasks
- Use @superpowers:verification-before-completion before claiming any task complete

## Success Criteria

- [ ] Recording panel appears in nettest.html
- [ ] Can select recording modes
- [ ] Can configure PiP and chart overlays
- [ ] Start recording captures video with all layers
- [ ] Sensor data collected during recording
- [ ] Stop recording saves to IndexedDB
- [ ] Can download video (.webm) and data (.json)
- [ ] No impact on network test performance
- [ ] Works on desktop Chrome/Firefox/Safari
- [ ] Works on iOS Safari with sensor permissions
