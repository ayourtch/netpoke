# Camera Integration Issue Discovery

## Task Overview
Compare the camera code in `tmp/camera-standalone-for-cross-check/` (working reference) with the integrated version in `client/src/recorder/` and `server/static/`. Find and document discrepancies following the process in `docs/issues/README.md`.

## Key Files to Compare

### Working Reference (Standalone)
**Location**: `tmp/camera-standalone-for-cross-check/`

Core files:
- `index.html` - Complete working HTML with sensor setup
- `src/lib.rs` - WASM exports and global state management  
- `src/app.rs` - Application state and initialization
- `src/ui.rs` - UI event handlers and initialization
- `src/sensors.rs` - SensorManager implementation
- `src/types.rs` - Data structures
- `src/canvas_renderer.rs` - Video rendering with overlays
- `src/storage.rs` - IndexedDB wrapper
- `src/media_streams.rs` - Camera/screen capture

### Integrated Version
**Location**: `client/src/` and `server/static/`

Key comparison points:
- `client/src/lib.rs` - WASM exports (search for `#[wasm_bindgen]` sensor callbacks)
- `client/src/recorder/` - Recorder subsystem (equivalent to standalone src/)
- `server/static/nettest.html` - Integrated HTML (compare with standalone index.html)

### Design Documents
- `docs/plans/` - Implementation and system design documents
- `docs/issues/session-summary-*.md` - Session summaries with analysis context

## What to Look For

### 1. Function Signature Mismatches
Compare WASM function exports between standalone and integrated:
- Check parameter counts, types, and order
- Verify JavaScript callers pass the correct arguments
- Pay special attention to optional parameters

Key sensor functions to verify: `on_gps_update()`, `on_orientation()`, `on_motion()`, `on_magnetometer()`

### 2. Missing Exports/Integrations
Check if functions exist in WASM but aren't imported in HTML:
- Look at `const { ... } = module;` import statements in HTML files
- Compare with `#[wasm_bindgen]` exports in Rust code
- Verify event listeners are registered for all sensors

### 3. Module Path Differences
The integrated version uses module prefixes:
- Standalone: `crate::utils::log()`
- Integrated: `crate::recorder::utils::log()`

All integrated types/functions are under `crate::recorder::*` namespace.

### 4. iOS-Specific Requirements
iOS Safari has strict requirements for sensor permissions:
- Event listeners MUST be added in same synchronous task as permission grant
- Cannot use `await` between permission request and adding listeners
- Check the `requestSensorPermissions()` pattern in standalone for the correct approach

### 5. State Management
Compare initialization and state patterns:
- Standalone: `Rc<RefCell<AppState>>` passed to closures
- Integrated: `thread_local! { RECORDER_STATE }` with lazy init

Check for initialization order issues or race conditions.

### 6. UI Initialization Timing
- Standalone: Eager initialization
- Integrated: Lazy initialization

Verify DOM elements exist before access.

### 7. Feature Completeness
Check if new features in integrated version are fully wired up:
- Chart overlay rendering functions
- PiP positioning options
- Test metadata capture

## How to Document Issues

**See `docs/issues/README.md` for the complete issue tracking process**, including:
- How to find the next issue number
- Issue file naming convention
- Complete issue template
- Workflow for creating and resolving issues
- Priority guidelines

**Key points**:
- Be specific with file paths and context
- Include code snippets showing both wrong and correct versions
- Explain WHY it's wrong, not just WHAT
- Provide actionable implementation steps
- Consider iOS compatibility for sensor-related issues

## Testing Strategy

After documenting issues:
1. Build and test on desktop browser first
2. Test on iOS Safari (sensor permission handling is iOS-specific)
3. Check browser console for JavaScript errors
4. Verify sensor data in recordings (download motion data JSON)
5. Test all source types: camera, screen, combined
