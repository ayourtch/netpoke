# Rust WebAssembly Conversion Design

## Overview

Convert the existing JavaScript camera/screen recording application to Rust WebAssembly while maintaining exact look, feel, and functionality.

**Date:** 2026-01-27
**Status:** Design Approved

## Goals

- Full Rust/WASM rewrite using wasm-pack + vanilla approach
- Preserve exact UI appearance and behavior
- Maintain all three recording modes: camera only, screen only, screen + camera (PiP)
- Keep all features: marquee animation, PiP controls, IndexedDB storage

## Architecture

### Build Tooling

- **wasm-pack**: Builds Rust to WebAssembly module
- **web-sys**: Web API bindings for DOM, Canvas, MediaStreams
- **wasm-bindgen**: JavaScript interop layer
- **wasm-bindgen-futures**: Async/await support for Promises

### Project Structure

```
camera/
├── src/
│   ├── lib.rs                 # WASM entry point, initialization
│   ├── app.rs                 # Main application logic & state
│   ├── recorder.rs            # Recording state machine
│   ├── canvas_renderer.rs     # Canvas compositing (PiP, marquee)
│   ├── media_streams.rs       # Camera/screen stream management
│   ├── storage.rs             # IndexedDB wrapper
│   ├── ui.rs                  # DOM manipulation, event handlers
│   └── utils.rs               # Metrics, timing utilities
├── js/
│   ├── media_recorder.js      # MediaRecorder JS interop helpers
│   └── indexed_db.js          # IndexedDB JS interop helpers
├── index.html                 # Same HTML structure (styles preserved)
├── Cargo.toml
└── package.json
```

## Component Details

### 1. lib.rs - Entry Point

**Purpose:** WASM entry point and initialization

**Responsibilities:**
- Export `start()` function called from HTML
- Set up panic hooks for debugging (`console_error_panic_hook`)
- Initialize `App` struct
- Register all event listeners
- Open IndexedDB connection
- Load saved recordings list

**Key exports:**
```rust
#[wasm_bindgen(start)]
pub fn main() {
    // Setup panic hook
}

#[wasm_bindgen]
pub async fn start() -> Result<(), JsValue> {
    // Initialize app
}
```

### 2. app.rs - Application State

**Purpose:** Central state manager coordinating all components

**State structure:**
```rust
struct AppState {
    recorder: Option<Recorder>,
    current_source_type: Option<SourceType>,
    recordings: Vec<Recording>,
    ui: UiController,
    db: IndexedDbWrapper,
}

enum SourceType {
    Camera,
    Screen,
    Combined,
}
```

**Key methods:**
- `start_tracking(source_type)`: Initiates recording flow
- `stop_tracking()`: Stops recording and saves
- `update_metrics()`: Updates UI with frame count, duration, size
- `refresh_recordings_list()`: Reloads from IndexedDB

**State sharing:** Uses `Rc<RefCell<AppState>>` pattern for shared mutable state across closures

### 3. recorder.rs - Recording State Machine

**Purpose:** Manages recording lifecycle and MediaRecorder coordination

**Responsibilities:**
- Track frame count, duration, start time (UTC)
- Manage MediaRecorder via JS interop
- Accumulate recorded data chunks
- Coordinate with canvas renderer for frame updates
- Trigger save on stop

**Key fields:**
```rust
struct Recorder {
    media_recorder_id: String,
    recorded_chunks: Vec<JsValue>,  // Blob chunks
    frame_count: u32,
    start_time: f64,
    start_time_utc: String,
    current_source_type: SourceType,
    render_interval_handle: Option<i32>,
    metrics_interval_handle: Option<i32>,
}
```

**Lifecycle:**
1. Create with canvas stream
2. Start MediaRecorder via JS
3. Accumulate chunks via `ondataavailable` callback
4. Stop and finalize on user action

### 4. canvas_renderer.rs - Rendering Pipeline

**Purpose:** Core rendering logic for all display modes

**Responsibilities:**
- Draw camera/screen/combined modes to canvas
- PiP positioning calculation (4 corner positions)
- Border and shadow drawing for camera overlay
- Marquee text animation
- Frame timing at 30 FPS (33ms intervals)

**Rendering modes:**

**Camera mode:**
```rust
fn render_camera(ctx: &CanvasRenderingContext2d, camera_video: &HtmlVideoElement) {
    canvas.set_width(camera_video.video_width());
    canvas.set_height(camera_video.video_height());
    ctx.draw_image_with_html_video_element(&camera_video, 0.0, 0.0);
}
```

**Screen mode:**
```rust
fn render_screen(ctx: &CanvasRenderingContext2d, screen_video: &HtmlVideoElement) {
    canvas.set_width(screen_video.video_width());
    canvas.set_height(screen_video.video_height());
    ctx.draw_image_with_html_video_element(&screen_video, 0.0, 0.0);
}
```

**Combined mode (PiP):**
```rust
fn render_combined(
    ctx: &CanvasRenderingContext2d,
    screen_video: &HtmlVideoElement,
    camera_video: &HtmlVideoElement,
    pip_position: &str,
    pip_size_percent: f64
) {
    // 1. Draw screen as background
    ctx.draw_image_with_html_video_element(&screen_video, 0.0, 0.0, width, height);

    // 2. Calculate PiP dimensions
    let pip_width = canvas_width * (pip_size_percent / 100.0);
    let pip_height = (camera_height / camera_width) * pip_width;
    let margin = 20.0;

    // 3. Calculate position based on dropdown
    let (x, y) = match pip_position {
        "bottom-right" => (canvas_width - pip_width - margin,
                          canvas_height - pip_height - margin),
        "bottom-left" => (margin, canvas_height - pip_height - margin),
        "top-right" => (canvas_width - pip_width - margin, margin),
        "top-left" => (margin, margin),
        _ => (canvas_width - pip_width - margin, canvas_height - pip_height - margin),
    };

    // 4. Draw shadow
    ctx.set_shadow_color("rgba(0,0,0,0.5)");
    ctx.set_shadow_blur(10.0);
    ctx.set_fill_style(&JsValue::from_str("#000"));
    ctx.fill_rect(x - 2.0, y - 2.0, pip_width + 4.0, pip_height + 4.0);
    ctx.set_shadow_blur(0.0);

    // 5. Draw camera feed
    ctx.draw_image_with_html_video_element_and_dw_and_dh(
        &camera_video, x, y, pip_width, pip_height
    );

    // 6. Draw border
    ctx.set_stroke_style(&JsValue::from_str("#fff"));
    ctx.set_line_width(2.0);
    ctx.stroke_rect(x, y, pip_width, pip_height);

    // 7. Draw marquee
    draw_marquee(ctx, canvas_width);
}
```

**Marquee animation:**
```rust
fn draw_marquee(ctx: &CanvasRenderingContext2d, canvas_width: f64) {
    let text = "https://stdio.be/cast - record your own screencast here - completely free";
    let font_size = 20.0;
    let marquee_y = 30.0;

    ctx.set_font(&format!("bold {}px system-ui, -apple-system, sans-serif", font_size));
    ctx.set_fill_style(&JsValue::from_str("rgba(255, 255, 255, 0.9)"));
    ctx.set_text_align("center");

    // Scrolling effect at 0.123 pixels/ms
    let now = js_sys::Date::now();
    let text_width = ctx.measure_text(text).unwrap().width();
    let scroll_x = (now * 0.123) % (text_width + 200.0);
    let draw_x = canvas_width / 2.0 - scroll_x + 100.0;
    let draw_x2 = draw_x + text_width + 200.0;

    // Draw with shadow for readability
    ctx.set_shadow_color("rgba(0, 0, 0, 0.8)");
    ctx.set_shadow_blur(4.0);
    ctx.fill_text(text, draw_x, marquee_y + 12.0);
    ctx.fill_text(text, draw_x2, marquee_y + 12.0);
    ctx.set_shadow_blur(0.0);
}
```

### 5. media_streams.rs - Stream Management

**Purpose:** Handle getUserMedia and getDisplayMedia calls

**Key functions:**
```rust
pub async fn get_camera_stream() -> Result<MediaStream, JsValue> {
    let navigator = web_sys::window().unwrap().navigator();
    let media_devices = navigator.media_devices()?;

    let constraints = MediaStreamConstraints::new();
    constraints.set_video(&create_camera_constraints());
    constraints.set_audio(&JsValue::TRUE);

    let promise = media_devices.get_user_media_with_constraints(&constraints)?;
    let stream = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(MediaStream::from(stream))
}

pub async fn get_screen_stream() -> Result<MediaStream, JsValue> {
    let navigator = web_sys::window().unwrap().navigator();
    let media_devices = navigator.media_devices()?;

    let constraints = DisplayMediaStreamConstraints::new();
    constraints.set_video(&create_screen_constraints());
    constraints.set_audio(&JsValue::TRUE);

    let promise = media_devices.get_display_media_with_constraints(&constraints)?;
    let stream = wasm_bindgen_futures::JsFuture::from(promise).await?;
    Ok(MediaStream::from(stream))
}

fn create_camera_constraints() -> JsValue {
    // facingMode: 'user'
    // width: { ideal: 1280 }
    // height: { ideal: 720 }
}

fn create_screen_constraints() -> JsValue {
    // width: { ideal: 1920 }
    // height: { ideal: 1080 }
    // frameRate: { ideal: 30 }
}
```

**Screen share stop detection:**
```rust
pub fn add_screen_stop_listener(
    stream: &MediaStream,
    callback: impl Fn() + 'static
) {
    let tracks = stream.get_video_tracks();
    if tracks.length() > 0 {
        let track = MediaStreamTrack::from(tracks.get(0));
        let closure = Closure::wrap(Box::new(callback) as Box<dyn Fn()>);
        track.add_event_listener_with_callback("ended", closure.as_ref().unchecked_ref());
        closure.forget();
    }
}
```

### 6. storage.rs - IndexedDB Wrapper

**Purpose:** Rust wrapper around IndexedDB via JS interop

**Interface:**
```rust
pub struct IndexedDbWrapper {
    db_name: String,
}

impl IndexedDbWrapper {
    pub async fn open() -> Result<Self, JsValue> {
        open_db_js().await?;
        Ok(Self { db_name: "CameraTrackingDB".to_string() })
    }

    pub async fn save_recording(
        &self,
        id: &str,
        blob: &web_sys::Blob,
        metadata: &RecordingMetadata
    ) -> Result<(), JsValue> {
        save_recording_js(id, blob, &metadata.to_js_value()).await
    }

    pub async fn get_all_recordings(&self) -> Result<Vec<Recording>, JsValue> {
        let js_recordings = get_all_recordings_js().await?;
        // Convert JS array to Vec<Recording>
    }

    pub async fn delete_recording(&self, id: &str) -> Result<(), JsValue> {
        delete_recording_js(id).await
    }
}

pub struct RecordingMetadata {
    pub frame_count: u32,
    pub duration: f64,
    pub mime_type: String,
    pub start_time_utc: String,
    pub end_time_utc: String,
    pub source_type: SourceType,
}
```

**JS interop declarations:**
```rust
#[wasm_bindgen(module = "/js/indexed_db.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn open_db_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn save_recording_js(
        id: &str,
        blob: &web_sys::Blob,
        metadata: &JsValue
    ) -> Result<(), JsValue>;

    #[wasm_bindgen(catch)]
    async fn get_all_recordings_js() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn delete_recording_js(id: &str) -> Result<(), JsValue>;
}
```

### 7. ui.rs - DOM Controller

**Purpose:** Direct DOM manipulation using web-sys

**Key responsibilities:**
- Element queries and caching
- Event listener registration
- Button state management
- Metrics display updates
- Recording list rendering

**Structure:**
```rust
pub struct UiController {
    // Cached elements
    status_el: HtmlElement,
    start_camera_btn: HtmlButtonElement,
    start_screen_btn: HtmlButtonElement,
    start_combined_btn: HtmlButtonElement,
    stop_btn: HtmlButtonElement,
    metrics_div: HtmlElement,
    pip_controls_div: HtmlElement,
    frames_el: HtmlElement,
    duration_el: HtmlElement,
    video_size_el: HtmlElement,
    source_type_el: HtmlElement,
    recordings_list_el: HtmlElement,
    pip_position_el: HtmlSelectElement,
    pip_size_el: HtmlInputElement,
    pip_size_label_el: HtmlElement,
}

impl UiController {
    pub fn new() -> Result<Self, JsValue> {
        let document = web_sys::window().unwrap().document().unwrap();

        Ok(Self {
            status_el: get_element_by_id(&document, "status")?,
            start_camera_btn: get_element_by_id(&document, "startCameraBtn")?,
            // ... other elements
        })
    }

    pub fn show_recording_state(&self, source_type: SourceType) {
        self.start_camera_btn.style().set_property("display", "none");
        self.start_screen_btn.style().set_property("display", "none");
        self.start_combined_btn.style().set_property("display", "none");
        self.stop_btn.style().set_property("display", "block");
        self.metrics_div.style().set_property("display", "block");

        let pip_display = if matches!(source_type, SourceType::Combined) {
            "block"
        } else {
            "none"
        };
        self.pip_controls_div.style().set_property("display", pip_display);

        let source_label = match source_type {
            SourceType::Camera => "Camera",
            SourceType::Screen => "Screen",
            SourceType::Combined => "Screen + Camera (PiP)",
        };
        self.source_type_el.set_text_content(Some(source_label));
        self.status_el.set_text_content(Some("Recording..."));
    }

    pub fn show_ready_state(&self) {
        self.start_camera_btn.style().set_property("display", "block");
        self.start_screen_btn.style().set_property("display", "block");
        self.start_combined_btn.style().set_property("display", "block");
        self.stop_btn.style().set_property("display", "none");
        self.metrics_div.style().set_property("display", "none");
        self.pip_controls_div.style().set_property("display", "none");
        self.status_el.set_text_content(Some("Ready to start"));
    }

    pub fn update_metrics(&self, frames: u32, duration: f64, video_size_mb: f64) {
        self.frames_el.set_text_content(Some(&frames.to_string()));
        self.duration_el.set_text_content(Some(&format!("{:.1}s", duration)));
        self.video_size_el.set_text_content(Some(&format!("{:.2} MB", video_size_mb)));
    }

    pub fn render_recordings_list(&self, recordings: &[Recording]) {
        if recordings.is_empty() {
            self.recordings_list_el.set_inner_html(
                "<p style=\"color:#888;\">No recordings yet</p>"
            );
            return;
        }

        let html = recordings.iter().map(|rec| {
            let date = js_sys::Date::new(&JsValue::from(rec.timestamp));
            let size_mb = rec.blob_size as f64 / (1024.0 * 1024.0);
            let source_class = format!("source-{}", rec.metadata.source_type.as_str());
            let source_label = rec.metadata.source_type.display_name();

            format!(
                r#"
                <div class="recording-item">
                    <div class="data">ID: {} <span class="source-label {}">{}</span></div>
                    <div class="data">Date: {}</div>
                    <div class="data">Duration: {:.1}s</div>
                    <div class="data">Frames: {}</div>
                    <div class="data">Size: {:.2} MB</div>
                    <button onclick="downloadVideo('{}')">Download Video</button>
                    <button class="danger" onclick="deleteRecordingById('{}')">Delete</button>
                </div>
                "#,
                rec.id, source_class, source_label,
                date.to_locale_string("en-US", &JsValue::UNDEFINED).as_string().unwrap(),
                rec.metadata.duration, rec.metadata.frame_count, size_mb,
                rec.id, rec.id
            )
        }).collect::<Vec<_>>().join("");

        self.recordings_list_el.set_inner_html(&html);
    }

    pub fn register_event_listeners(
        &self,
        app_state: Rc<RefCell<AppState>>
    ) -> Result<(), JsValue> {
        // Start camera button
        {
            let app = app_state.clone();
            let closure = Closure::wrap(Box::new(move || {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = app.borrow_mut().start_tracking(SourceType::Camera).await;
                });
            }) as Box<dyn Fn()>);
            self.start_camera_btn.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref()
            )?;
            closure.forget();
        }

        // Start screen button
        {
            let app = app_state.clone();
            let closure = Closure::wrap(Box::new(move || {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = app.borrow_mut().start_tracking(SourceType::Screen).await;
                });
            }) as Box<dyn Fn()>);
            self.start_screen_btn.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref()
            )?;
            closure.forget();
        }

        // Start combined button
        {
            let app = app_state.clone();
            let closure = Closure::wrap(Box::new(move || {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = app.borrow_mut().start_tracking(SourceType::Combined).await;
                });
            }) as Box<dyn Fn()>);
            self.start_combined_btn.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref()
            )?;
            closure.forget();
        }

        // Stop button
        {
            let app = app_state.clone();
            let closure = Closure::wrap(Box::new(move || {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = app.borrow_mut().stop_tracking().await;
                });
            }) as Box<dyn Fn()>);
            self.stop_btn.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref()
            )?;
            closure.forget();
        }

        // PiP size slider
        {
            let label_el = self.pip_size_label_el.clone();
            let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
                let target = event.target().unwrap();
                let input = target.dyn_into::<HtmlInputElement>().unwrap();
                let value = input.value();
                label_el.set_text_content(Some(&format!("{}%", value)));
            }) as Box<dyn Fn(web_sys::Event)>);
            self.pip_size_el.add_event_listener_with_callback(
                "input",
                closure.as_ref().unchecked_ref()
            )?;
            closure.forget();
        }

        Ok(())
    }
}
```

### 8. utils.rs - Utilities

**Purpose:** Helper functions and utilities

**Contents:**
- Time formatting
- Size formatting (bytes to MB)
- Error handling helpers
- Logging macros

```rust
pub fn format_duration(seconds: f64) -> String {
    format!("{:.1}s", seconds)
}

pub fn format_size_mb(bytes: usize) -> String {
    format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
}

pub fn current_timestamp_ms() -> f64 {
    js_sys::Date::now()
}

pub fn current_timestamp_utc() -> String {
    let date = js_sys::Date::new_0();
    date.to_iso_string().as_string().unwrap()
}
```

## JavaScript Interop Layer

### js/media_recorder.js

**Purpose:** MediaRecorder wrapper for Rust

```javascript
// Registry to hold MediaRecorder instances
const recorders = new Map();
let nextId = 0;

export function createMediaRecorder(stream, mimeType) {
    const id = `recorder_${nextId++}`;
    const chunks = [];

    // Codec detection
    const mp4Codecs = [
        'video/mp4;codecs=avc1.42E01E',
        'video/mp4;codecs=avc1.4D401E',
        'video/mp4',
        'video/webm;codecs=vp8.0',
        'video/webm;codecs=vp9',
        'video/webm'
    ];

    const supported = mp4Codecs.filter(codec =>
        MediaRecorder.isTypeSupported(codec)
    );

    if (supported.length === 0) {
        throw new Error('No supported recording codecs found');
    }

    // iOS Safari handling
    const isIOS = /iPad|iPhone|iPod/.test(navigator.userAgent);
    let selectedMimeType;

    if (isIOS) {
        const webmCodec = supported.find(c => c.includes('webm'));
        selectedMimeType = webmCodec || supported[0];
    } else {
        const mp4Codec = supported.find(c => c.includes('mp4'));
        selectedMimeType = mp4Codec || supported[0];
    }

    const recorder = new MediaRecorder(stream, {
        mimeType: selectedMimeType,
        videoBitsPerSecond: 4000000
    });

    recorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
            chunks.push(event.data);
        }
    };

    recorders.set(id, { recorder, chunks, mimeType: selectedMimeType });

    return { id, mimeType: selectedMimeType };
}

export function startRecorder(id) {
    const entry = recorders.get(id);
    if (!entry) throw new Error(`Recorder ${id} not found`);
    entry.recorder.start(1000);
}

export function stopRecorder(id) {
    const entry = recorders.get(id);
    if (!entry) throw new Error(`Recorder ${id} not found`);

    return new Promise((resolve) => {
        entry.recorder.onstop = () => {
            const blob = new Blob(entry.chunks, { type: entry.mimeType });
            resolve(blob);
            recorders.delete(id);
        };
        entry.recorder.stop();
    });
}

export function getRecorderState(id) {
    const entry = recorders.get(id);
    return entry ? entry.recorder.state : 'inactive';
}

export function getChunksSize(id) {
    const entry = recorders.get(id);
    if (!entry) return 0;
    return entry.chunks.reduce((sum, chunk) => sum + chunk.size, 0);
}
```

### js/indexed_db.js

**Purpose:** IndexedDB operations for Rust

```javascript
let db = null;

export async function openDb() {
    return new Promise((resolve, reject) => {
        const request = indexedDB.open('CameraTrackingDB', 2);

        request.onerror = () => reject(request.error);
        request.onsuccess = () => {
            db = request.result;
            resolve();
        };

        request.onupgradeneeded = (event) => {
            const db = event.target.result;
            if (!db.objectStoreNames.contains('recordings')) {
                db.createObjectStore('recordings', { keyPath: 'id' });
            }
        };
    });
}

export async function saveRecording(id, videoBlob, metadata) {
    if (!db) throw new Error('Database not initialized');

    const recording = {
        id,
        videoBlob,
        metadata,
        timestamp: Date.now()
    };

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readwrite');
        const store = transaction.objectStore('recordings');
        const request = store.put(recording);

        request.onsuccess = () => resolve();
        request.onerror = () => reject(request.error);
    });
}

export async function getAllRecordings() {
    if (!db) throw new Error('Database not initialized');

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readonly');
        const store = transaction.objectStore('recordings');
        const request = store.getAll();

        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
}

export async function deleteRecording(id) {
    if (!db) throw new Error('Database not initialized');

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readwrite');
        const store = transaction.objectStore('recordings');
        const request = store.delete(id);

        request.onsuccess = () => resolve();
        request.onerror = () => reject(request.error);
    });
}

export async function getRecording(id) {
    if (!db) throw new Error('Database not initialized');

    return new Promise((resolve, reject) => {
        const transaction = db.transaction(['recordings'], 'readonly');
        const store = transaction.objectStore('recordings');
        const request = store.get(id);

        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
}
```

## Data Flow

### Recording Flow

1. **User clicks button** → `ui.rs` event handler triggers
2. **Start tracking** → `app.start_tracking(source_type)` called
3. **Acquire streams** → `media_streams.rs` gets camera/screen via getUserMedia/getDisplayMedia
4. **Create recorder** → `recorder.rs` instantiates with canvas stream
5. **Start render loop** → `canvas_renderer.rs` begins 30 FPS rendering (setInterval equivalent)
6. **Start MediaRecorder** → Via JS interop, begins recording canvas
7. **Update metrics** → Every 100ms update frame count, duration, size
8. **User stops** → Click stop button
9. **Finalize recording** → `recorder.rs` stops MediaRecorder, gets final blob
10. **Save to IndexedDB** → `storage.rs` saves blob + metadata
11. **Update UI** → Refresh recordings list, reset to ready state

### PiP Rendering Flow (Combined Mode)

1. **Get dimensions** → Read canvas size from screen video
2. **Calculate PiP size** → `pip_width = canvas_width * (slider_value / 100)`
3. **Calculate aspect ratio** → `pip_height = (camera_h / camera_w) * pip_width`
4. **Determine position** → Based on dropdown selection (4 corners)
5. **Draw background** → Screen video full canvas
6. **Draw shadow** → Black rect with blur at PiP position
7. **Draw camera** → Video feed at calculated position/size
8. **Draw border** → White 2px stroke around camera
9. **Draw marquee** → Scrolling text at top with shadow

## Dependencies

### Cargo.toml

```toml
[package]
name = "camera-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = [
    "console",
    "Window",
    "Document",
    "Element",
    "HtmlElement",
    "HtmlButtonElement",
    "HtmlInputElement",
    "HtmlSelectElement",
    "HtmlVideoElement",
    "HtmlCanvasElement",
    "CanvasRenderingContext2d",
    "MediaDevices",
    "MediaStream",
    "MediaStreamTrack",
    "MediaStreamConstraints",
    "DisplayMediaStreamOptions",
    "Navigator",
    "Blob",
    "BlobPropertyBag",
    "Event",
    "EventTarget",
    "CssStyleDeclaration",
    "TextMetrics",
]}
js-sys = "0.3"
console_error_panic_hook = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

### package.json

```json
{
  "name": "camera-wasm",
  "version": "0.1.0",
  "scripts": {
    "build": "wasm-pack build --target web --out-dir pkg",
    "serve": "python3 -m http.server 8080"
  },
  "devDependencies": {
    "wasm-pack": "^0.12.1"
  }
}
```

## HTML Integration

The index.html will remain largely the same, with CSS unchanged. Only the script section changes:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <!-- Same meta, title, and styles as original -->
</head>
<body>
    <!-- Same HTML structure as original -->

    <script type="module">
        import init, { start } from './pkg/camera_wasm.js';

        async function run() {
            await init();
            await start();
        }

        run();
    </script>
</body>
</html>
```

## Implementation Phases

### Phase 1: Project Setup
- Initialize Cargo project with wasm-pack
- Add dependencies to Cargo.toml
- Create basic lib.rs with panic hook
- Verify WASM builds successfully

### Phase 2: Core Infrastructure
- Implement app.rs with AppState structure
- Implement ui.rs with DOM queries and element caching
- Set up event listener registration
- Test basic button click handling

### Phase 3: Media Streams
- Implement media_streams.rs
- Get camera stream working
- Get screen stream working
- Add screen share stop detection

### Phase 4: Canvas Rendering
- Implement canvas_renderer.rs
- Camera mode rendering
- Screen mode rendering
- Combined mode with PiP
- Marquee animation

### Phase 5: Recording
- Create JS interop for MediaRecorder (js/media_recorder.js)
- Implement recorder.rs
- Test recording in all three modes
- Verify codec selection works

### Phase 6: Storage
- Create JS interop for IndexedDB (js/indexed_db.js)
- Implement storage.rs wrapper
- Test save/load/delete operations
- Implement recordings list rendering

### Phase 7: Metrics & Polish
- Implement metrics updates (frame count, duration, size)
- PiP controls (position dropdown, size slider)
- Download and delete functionality
- Error handling and user feedback

### Phase 8: Testing & Refinement
- Test all three recording modes
- Verify PiP positioning in all 4 corners
- Verify PiP size adjustment
- Test on different browsers (Chrome, Firefox, Safari)
- Test on iOS Safari specifically
- Verify marquee scrolls correctly
- Verify recordings save and load correctly

## Success Criteria

- All three recording modes work identically to original
- UI looks pixel-perfect identical
- PiP positioning and sizing works in all configurations
- Marquee animation scrolls at same speed (0.123 px/ms)
- Recordings save to IndexedDB correctly
- Download and delete work correctly
- Works on desktop Chrome, Firefox, Safari
- Works on iOS Safari
- No console errors or warnings

## Known Challenges

1. **MediaRecorder API complexity**: Handled via JS interop layer
2. **IndexedDB Promise handling**: Handled via JS interop layer
3. **Canvas captureStream()**: May need fallbacks for iOS Safari
4. **Codec detection**: Handled in JS layer with same logic as original
5. **Closure memory management**: Use `.forget()` pattern carefully for event listeners
6. **State sharing**: Use `Rc<RefCell<>>` pattern for shared mutable state

## Future Enhancements (Out of Scope)

- WebCodecs API for more direct video encoding
- WebGPU for canvas rendering acceleration
- Rust-native IndexedDB via rexie crate
- Service Worker for offline functionality
- Video editing features
