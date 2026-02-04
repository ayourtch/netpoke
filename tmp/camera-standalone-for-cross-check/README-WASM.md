# Camera WASM - Rust WebAssembly Version

Rust WebAssembly version of the camera/screen recording application. Maintains exact functionality and appearance of the original JavaScript version.

## Features

- **Camera-only recording**: Record from webcam with audio
- **Screen-only recording**: Record screen/window with audio
- **Combined PiP mode**: Screen recording with camera overlay (Picture-in-Picture)
  - Adjustable PiP size (10-40%)
  - 4 position options (all 4 corners)
  - Border and shadow effects
- **Scrolling marquee**: Animated text overlay at top of screen recordings
- **IndexedDB storage**: Recordings saved locally in browser
- **Download/Delete**: Full recording management

## Building

### Prerequisites

Install Rust and wasm-pack:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install wasm-pack
```

### Build for Development

```bash
wasm-pack build --target web --out-dir pkg
```

### Build for Production

```bash
wasm-pack build --target web --out-dir pkg --release
```

The production build is optimized for size with LTO enabled.

## Running

Serve the directory with any HTTP server:

```bash
# Using Python
python3 -m http.server 8080

# Using Node.js
npx http-server -p 8080
```

Open http://localhost:8080 in your browser.

## Architecture

### Rust Modules

- **lib.rs**: WASM entry point and global functions
- **app.rs**: Application state and recording lifecycle
- **ui.rs**: DOM manipulation and event handling
- **canvas_renderer.rs**: Canvas compositing with PiP and marquee
- **media_streams.rs**: Camera/screen capture via getUserMedia/getDisplayMedia
- **recorder.rs**: MediaRecorder wrapper (via JS interop)
- **storage.rs**: IndexedDB wrapper (via JS interop)
- **types.rs**: Shared type definitions
- **utils.rs**: Utility functions (timestamps, formatting)

### JavaScript Interop

- **js/media_recorder.js**: MediaRecorder API wrapper with codec detection
- **js/indexed_db.js**: IndexedDB operations for recording storage

### Web APIs Used

- MediaDevices (getUserMedia, getDisplayMedia)
- MediaRecorder
- Canvas API
- IndexedDB
- MediaStream

## Technical Details

### Canvas Rendering

All three modes render to a canvas at 30 FPS:
- Camera mode: Direct video feed rendering
- Screen mode: Direct screen feed rendering
- Combined mode: Screen background + camera PiP overlay + marquee text

The canvas is captured as a MediaStream and recorded using MediaRecorder.

### PiP Implementation

In combined mode, the camera feed is composited onto the screen recording:
1. Screen video drawn full canvas
2. Camera video sized and positioned based on controls
3. Shadow and border effects applied
4. Marquee text scrolls across top (0.123 pixels/ms)

### Codec Selection

MediaRecorder codec selection prioritizes:
- Desktop: H.264 MP4 (best compatibility)
- iOS Safari: WebM (better compatibility with video element capture)

### Storage

Recordings stored in IndexedDB with metadata:
- Frame count
- Duration
- MIME type
- Start/end timestamps (UTC)
- Source type (camera/screen/combined)

## Browser Compatibility

Tested on:
- Chrome/Chromium (desktop)
- Firefox (desktop)
- Safari (desktop and iOS)

Requires browser support for:
- WebAssembly
- MediaRecorder API
- canvas.captureStream()
- getDisplayMedia (for screen recording)

## Development

### Project Structure

```
camera/
├── src/                    # Rust source code
│   ├── lib.rs             # WASM entry point
│   ├── app.rs             # Application state
│   ├── ui.rs              # DOM controller
│   ├── canvas_renderer.rs # Canvas rendering
│   ├── media_streams.rs   # Media capture
│   ├── recorder.rs        # Recording logic
│   ├── storage.rs         # IndexedDB wrapper
│   ├── types.rs           # Type definitions
│   └── utils.rs           # Utilities
├── js/                    # JavaScript interop
│   ├── media_recorder.js  # MediaRecorder wrapper
│   └── indexed_db.js      # IndexedDB wrapper
├── pkg/                   # Build output (generated)
├── index.html             # Main HTML
├── Cargo.toml             # Rust dependencies
└── package.json           # Build scripts
```

### Dependencies

Main dependencies:
- `wasm-bindgen`: Rust/JS interop
- `web-sys`: Web API bindings
- `js-sys`: JavaScript type bindings
- `wasm-bindgen-futures`: Async/await support
- `serde` + `serde-wasm-bindgen`: Serialization

See `Cargo.toml` for full list with versions.

## Differences from JavaScript Version

### Functionally Identical

The Rust/WASM version maintains identical functionality:
- All three recording modes work the same
- UI looks and behaves identically
- Same codec selection logic
- Same canvas rendering (PiP, marquee)
- Same storage format in IndexedDB

### Implementation Differences

- **Language**: Rust instead of JavaScript
- **Async**: Uses Rust async/await with wasm-bindgen-futures
- **State**: Rust structs with Rc<RefCell<>> instead of JS variables
- **Types**: Strong typing throughout
- **Modules**: Separated into logical Rust modules

## Performance

WASM binary size (release build): ~300KB (optimized with `opt-level = "z"` and LTO)

Canvas rendering maintains 30 FPS for smooth recording across all modes.

## License

Same as original project.
