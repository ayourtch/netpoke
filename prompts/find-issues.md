# Camera Integration Issue Discovery Process

## Task Overview
Look at recent commits that attempted to fuse the camera code (in `tmp/camera-standalone-for-cross-check/`) with NetPoke's client network tester. Find and document discrepancies between the working standalone implementation and the integrated version.

## Key Files to Compare

### Working Reference (Standalone)
**Location**: `tmp/camera-standalone-for-cross-check/`

Core files to review:
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
- `client/src/lib.rs` - WASM exports (lines 1928-2141 for sensor callbacks)
- `client/src/recorder/` - Recorder subsystem (equivalent to standalone src/)
- `server/static/nettest.html` - Integrated HTML (compare with standalone index.html)
- `server/static/camera-tracker.html` - Legacy approach, may be outdated

### Design Documents
- `docs/plans/2025-12-05-network-measurement-implementation.md` - Implementation plan
- `docs/plans/2025-12-05-network-measurement-system-design.md` - System design
- `docs/issues/session-summary-2026-02-04.md` - Recent work summary

## What to Look For

### 1. Function Signature Mismatches
Compare WASM function exports between standalone and integrated:
- Check parameter counts, types, and order
- Verify JavaScript callers pass the right number of arguments
- Pay special attention to optional parameters

**Critical Functions**:
- `on_gps_update()` - 7 parameters (check order: latitude, longitude, altitude, accuracy...)
- `on_orientation()` - 4 parameters (alpha, beta, gamma, absolute)
- `on_motion()` - 11 parameters in integrated vs 9 in standalone! (timestamp added)
- `on_magnetometer()` - 4 parameters (alpha, beta, gamma, absolute)

### 2. Missing Exports/Integrations
Check if functions exist in WASM but aren't imported in HTML:
- Look at `const { ... } = module;` import statements in HTML
- Compare with `#[wasm_bindgen]` exports in lib.rs
- Check if event listeners are registered for sensors

### 3. Path Discrepancies
The integrated version uses module prefixes:
- Standalone: `crate::utils::log()`
- Integrated: `crate::recorder::utils::log()`

All integrated types/functions are under `crate::recorder::*` namespace.

### 4. iOS-Specific Issues
iOS Safari has strict requirements for sensor permissions:
- Event listeners MUST be added in same synchronous task as permission grant
- Cannot use `await` between permission and adding listeners
- Check `requestSensorPermissions()` pattern in standalone (lines 349-401)

### 5. State Management Differences
- Standalone: `Rc<RefCell<AppState>>` passed to closures
- Integrated: `thread_local! { RECORDER_STATE }` with lazy init

Check for initialization order issues or race conditions.

### 6. UI Initialization
- Standalone: Eager initialization in `AppState::new()`
- Integrated: Lazy initialization via `init_recorder_panel()`

Verify DOM elements exist before access.

### 7. Module Path Differences
All paths in integrated code have `recorder::` prefix:
```rust
// Standalone
use crate::types::*;
use crate::utils::log;

// Integrated  
use crate::recorder::types::*;
use crate::recorder::utils::log;
```

### 8. Additional Features in Integrated
The integrated version has extra features not in standalone:
- `PipPosition` enum for chart positioning
- `chart_included`, `chart_type`, `test_metadata` fields in RecordingMetadata
- `render_chart_overlay()` function for Chart.js compositing

Check if these features are fully wired up.

## How to Document Issues

Follow the process in `docs/issues/README.md`:

1. **Find next issue number**: 
   ```bash
   ls docs/issues/open/ docs/issues/resolved/ | grep -oE '^[0-9]+' | sort -n | tail -1
   ```

2. **Create issue file**: `docs/issues/open/NNN-short-description.md`

3. **Use this template**:
   ```markdown
   # Issue NNN: Short Title
   
   ## Summary
   Brief description of the issue.
   
   ## Location
   - File: `path/to/file.rs`
   - Function/Line: specific location
   - Reference: working code location
   
   ## Current Behavior
   What currently happens (the bug or missing feature).
   
   ## Expected Behavior
   What should happen instead.
   
   ## Impact
   Priority: Critical/High/Medium/Low
   How this affects users or the system.
   
   ## Suggested Implementation
   Step-by-step fix with code examples.
   
   ## Related Issues
   References to related issues if any.
   
   ---
   *Created: YYYY-MM-DD*
   ```

4. **Be specific and actionable**:
   - Include exact line numbers
   - Show code snippets for both wrong and correct versions
   - Explain WHY it's wrong, not just WHAT is wrong
   - Provide complete implementation steps
   - Consider edge cases and iOS compatibility

5. **Issue Priority Guidelines**:
   - **Critical**: Breaks core functionality, blocks deployment
   - **High**: Significant bug, data loss, security issue, iOS-specific crash
   - **Medium**: Feature incomplete, degraded UX, workaround exists
   - **Low**: Polish, documentation, cleanup, very minor bugs

## Common Discrepancies Found

Based on previous analysis, watch for:

1. ✅ **Missing `on_magnetometer` import** - Function exists but not used in nettest.html
2. ✅ **Signature mismatch** - Standalone calls `on_magnetometer(alpha, beta, gamma)` but Rust expects 4 params
3. ✅ **Sensor permission timing** - iOS requires listeners added immediately after permission grant
4. ⚠️ **Chart compositing** - `render_chart_overlay()` exists but never called
5. ⚠️ **camera-tracker.html** - Outdated file with plain JS approach (no WASM)

## Testing Strategy

After documenting issues:
1. Build and run on desktop browser first
2. Test on iOS Safari (permission handling is iOS-specific)
3. Check browser console for JavaScript errors
4. Verify sensor data appears in recordings (download motion data JSON)
5. Test all three source types: camera, screen, combined

## Recent Work Context

Check `docs/issues/session-summary-2026-02-04.md` - it shows:
- 20 issues were previously worked on
- Issues 001-020 cover various integration problems
- Many issues already resolved (in docs/issues/resolved/)
- Issue numbering continues from 021+

Start your issue numbering from 021 or whatever is next available.

