# Camera Integration Issue Discovery - Session Summary

**Date**: 2026-02-04  
**Task**: Analyze camera integration between standalone reference and integrated NetPoke client

## Issues Discovered and Documented

This session found and documented **5 new issues** (021-025) related to camera integration discrepancies.

### High Priority Issues

#### Issue 021: Missing on_magnetometer Export in nettest.html
**Priority**: Medium  
**Impact**: Absolute compass heading data not collected from iOS devices

The `on_magnetometer` WASM function exists but is not imported or used in the integrated HTML. The standalone version properly sets up `deviceorientationabsolute` event listeners and calls this function, but the integrated nettest.html does not.

**Fix Required**: Add `on_magnetometer` to WASM imports and set up event listeners.

---

#### Issue 022: on_magnetometer Function Signature Mismatch  
**Priority**: High  
**Impact**: Will cause runtime errors when Issue 021 is fixed

The standalone JavaScript calls `on_magnetometer(alpha, beta, gamma)` with 3 parameters, but the Rust function signature expects 4 parameters including `absolute: bool`. This mismatch needs to be fixed before implementing Issue 021.

**Fix Required**: Update standalone HTML to pass 4th parameter; ensure integrated version uses correct signature.

---

#### Issue 024: Sensor Permission Not Called From User Gesture Context
**Priority**: High (iOS-specific)  
**Impact**: Sensor tracking may fail silently on iOS Safari

iOS Safari requires that device motion/orientation event listeners be added in the **same synchronous execution context** as the permission grant. Using `await` between permission request and listener registration breaks this requirement.

**Fix Required**: Restructure permission flow to add listeners immediately after permission grant, before any async operations.

---

### Medium Priority Issues

#### Issue 025: Missing Chart Compositing in Integrated Recorder
**Priority**: Medium  
**Impact**: Chart overlay feature appears to work but doesn't actually composite charts into video

The `render_chart_overlay()` function exists in canvas_renderer.rs, and UI controls exist for chart selection, but the rendering pipeline never calls this function. Charts are not actually composited into recordings.

**Fix Required**: Wire up chart rendering in `render_frame()` method, add chart state tracking, and integrate with UI controls.

---

### Low Priority Issues

#### Issue 023: camera-tracker.html Uses Outdated Approach
**Priority**: Low (Documentation/Cleanup)  
**Impact**: May confuse developers about which approach to use

The `server/static/camera-tracker.html` file implements camera tracking using plain JavaScript without WASM integration, duplicating effort and potentially misleading developers.

**Fix Required**: Either remove/rename as deprecated, or document clearly as a simple demo (not production code).

---

## Key Findings from Code Analysis

### Architecture Differences

| Aspect | Standalone | Integrated |
|--------|-----------|-----------|
| State Management | `Rc<RefCell<AppState>>` | `thread_local! { RECORDER_STATE }` |
| Initialization | Eager (on app create) | Lazy (on user action) |
| Module Paths | `crate::utils::*` | `crate::recorder::utils::*` |
| Event Listeners | Registered in AppState | Registered in init_recorder_panel() |

### Function Signature Discrepancies

**on_motion()** - Parameter count mismatch:
- **Standalone JavaScript**: 9 parameters (acceleration + gravity + rotation)
- **Integrated Rust**: 11 parameters (adds timestamp_utc and current_time at start)
- **Status**: ✅ Already correct in integrated nettest.html (timestamps added)

**on_magnetometer()** - Missing parameter:
- **Standalone JavaScript**: 3 parameters (alpha, beta, gamma)
- **Integrated Rust**: 4 parameters (adds absolute: bool)
- **Status**: ❌ Mismatch needs fixing (Issue 022)

**on_gps_update()** - Parameter order difference:
- **Standalone**: latitude, longitude, altitude, accuracy, altitudeAccuracy, heading, speed
- **Integrated**: latitude, longitude, accuracy, altitude, altitude_accuracy, heading, speed
- **Status**: ⚠️ Reordered but both work (different param names in Rust)

### Enhanced Features in Integrated Version

The integrated version includes features not in standalone:
- ✅ `PipPosition` enum for positioning overlays
- ✅ `chart_included`, `chart_type`, `test_metadata` in RecordingMetadata
- ✅ `render_chart_overlay()` for Chart.js compositing
- ⚠️ But chart compositing not wired up (Issue 025)

### Files Compared

Identical (except module paths):
- ✅ `sensors.rs` - SensorManager implementation
- ✅ `media_streams.rs` - Camera/screen capture
- ✅ `storage.rs` - IndexedDB wrapper
- ✅ `media_recorder.rs` (was `recorder.rs` in standalone)

Enhanced in integrated:
- ✅ `canvas_renderer.rs` - Added `render_chart_overlay()` method
- ✅ `types.rs` - Added PipPosition enum, extra RecordingMetadata fields
- ✅ `ui.rs` - Refactored into modular setup functions

Different architecture:
- `app.rs` (standalone) → `state.rs` (integrated) - Different state management approach
- `lib.rs` - Standalone exports all functions directly, integrated delegates to modules

## Comparison with Previous Session

The session summary from 2026-02-04 shows that **20 issues** (001-020) were previously addressed, with 16 resolved and 4 remaining open. This session adds **5 more issues** (021-025), bringing the total to **25 documented issues**.

Previous session focus:
- Audio track integration ✅ Fixed
- Sensor overlay toggle ✅ Fixed  
- Chart canvas elements ✅ Fixed
- Database naming ✅ Fixed
- Download functions ✅ Fixed

This session focus:
- Sensor callback integration (021, 022, 024)
- iOS permission handling (024)
- Chart rendering wiring (025)
- Code cleanup (023)

## Testing Recommendations

### Desktop Testing
1. ✅ Code compiles (`cargo build`)
2. ⚠️ Sensor callbacks need manual testing
3. ⚠️ Chart overlay compositing needs implementation + testing

### iOS Safari Testing (Critical)
- ❌ Issue 024 is iOS-specific - must test on actual iOS device
- Sensor permissions and event listener registration
- Magnetometer/compass data capture
- Recording with all sensor data

### Integration Testing
1. Camera-only recording with sensors
2. Screen-only recording with sensors  
3. Combined (PiP) recording with sensors
4. Chart overlay recording (once Issue 025 fixed)
5. Download video and motion data JSON
6. Verify sensor data completeness in JSON

## Updated Documentation

### Enhanced find-issues.md
The `prompts/find-issues.md` file has been significantly expanded with:
- Complete comparison guide for standalone vs integrated
- Detailed checklist of what to look for
- Common pitfalls and iOS-specific requirements
- Issue documentation template with examples
- Priority guidelines and testing strategy
- Context from recent work and session summaries

Future agents should find this guide much more helpful for discovering and documenting integration issues.

## Recommendations for Next Steps

### Immediate (High Priority)
1. **Fix Issue 022** - Update standalone to pass 4th parameter to on_magnetometer
2. **Implement Issue 021** - Add magnetometer integration to nettest.html
3. **Fix Issue 024** - Restructure sensor permission flow for iOS compatibility

### Short Term (Medium Priority)  
4. **Implement Issue 025** - Wire up chart compositing in render pipeline
5. Test thoroughly on iOS Safari with all sensor types

### Long Term (Low Priority)
6. **Resolve Issue 023** - Clean up or document camera-tracker.html

## Files Modified

### New Issue Documents
- `docs/issues/open/021-missing-on-magnetometer-export.md`
- `docs/issues/open/022-on-magnetometer-signature-mismatch.md`
- `docs/issues/open/023-camera-tracker-outdated-approach.md`
- `docs/issues/open/024-sensor-permission-user-gesture-context.md`
- `docs/issues/open/025-missing-chart-compositing.md`

### Enhanced Documentation
- `prompts/find-issues.md` - Comprehensive discovery and documentation guide

## Conclusion

This session successfully identified 5 new integration issues by systematically comparing the working standalone camera code with the integrated NetPoke client. The most critical findings relate to iOS Safari compatibility (Issues 022, 024) and missing feature wiring (Issues 021, 025).

All issues are documented with:
- ✅ Clear problem statement and location
- ✅ Current vs expected behavior
- ✅ Impact and priority assessment
- ✅ Step-by-step implementation guidance
- ✅ Code examples and references

The enhanced `find-issues.md` guide will help future engineers/LLMs perform similar analysis more efficiently.

---
*Session Completed: 2026-02-04*
