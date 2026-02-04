# Issue 028: Sensor Callback Signature Mismatch for Null Values

## Summary
The integrated sensor callbacks (`on_orientation`, `on_magnetometer`) use non-optional `f64` parameters and coalesce null values to `0` in JavaScript, while the reference implementation uses `Option<f64>` to properly represent missing sensor data. This means the WASM code cannot distinguish between actual zero values and missing/unavailable sensor data.

## Location
- Files: 
  - `client/src/lib.rs` (lines 1955, 2006) - WASM function signatures
  - `server/static/nettest.html` (lines ~2658, ~2666) - JavaScript callers

## Current Behavior

**In `client/src/lib.rs`:**
```rust
#[wasm_bindgen]
pub fn on_orientation(alpha: f64, beta: f64, gamma: f64, absolute: bool) {
    // ...
    let orientation_data = recorder::types::OrientationData {
        alpha: Some(alpha),
        beta: Some(beta),
        gamma: Some(gamma),
        absolute,
    };
}

#[wasm_bindgen]
pub fn on_magnetometer(alpha: f64, beta: f64, gamma: f64, absolute: bool) {
    // ...
    let mag_data = recorder::types::OrientationData {
        alpha: Some(alpha),
        beta: Some(beta),
        gamma: Some(gamma),
        absolute,
    };
}
```

**In `server/static/nettest.html`:**
```javascript
orientationListener = (event) => {
    on_orientation(
        event.alpha || 0,      // ❌ Null becomes 0
        event.beta || 0,       // ❌ Null becomes 0
        event.gamma || 0,      // ❌ Null becomes 0
        event.absolute || false
    );
};

magnetometerListener = (event) => {
    if (on_magnetometer && event.alpha !== null) {
        on_magnetometer(
            event.alpha || 0,   // ✓ Already checked for null, but still coalesces
            event.beta || 0,
            event.gamma || 0,
            true
        );
    }
};
```

## Expected Behavior

The reference implementation properly handles null values:

**In `tmp/camera-standalone-for-cross-check/src/lib.rs`:**
```rust
#[wasm_bindgen]
pub fn on_orientation(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
    absolute: bool,
) {
    let orientation = crate::types::OrientationData {
        alpha,  // Preserves None for unavailable data
        beta,
        gamma,
        absolute,
    };
    // ...
}

#[wasm_bindgen]
pub fn on_magnetometer(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
) {
    let magnetometer = crate::types::OrientationData {
        alpha,  // Preserves None for unavailable data
        beta,
        gamma,
        absolute: true,
    };
    // ...
}
```

This allows the WASM code to properly distinguish:
- `Some(0.0)` - sensor value is actually zero
- `None` - sensor data is unavailable

## Impact

**Medium** - This is a data quality issue that affects:
1. **Motion data accuracy**: Zero values will be recorded even when sensors aren't available
2. **Debugging**: Can't distinguish between "sensor says 0" vs "sensor not available"
3. **Future features**: Any logic that needs to detect sensor availability will fail
4. **Cross-platform compatibility**: Some devices don't provide certain sensor axes; currently we'd record fake zeros

## Suggested Implementation

### Option 1: Update WASM signatures (Recommended)

**In `client/src/lib.rs`:**
```rust
#[wasm_bindgen]
pub fn on_orientation(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
    absolute: bool,
) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            let orientation_data = recorder::types::OrientationData {
                alpha,
                beta,
                gamma,
                absolute,
            };
            mgr.update_orientation(orientation_data);
        }
    }
}

#[wasm_bindgen]
pub fn on_magnetometer(
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
    absolute: bool,
) {
    if let Ok(mut manager_guard) = SENSOR_MANAGER.lock() {
        if let Some(ref mut mgr) = *manager_guard {
            let mag_data = recorder::types::OrientationData {
                alpha,
                beta,
                gamma,
                absolute,
            };
            mgr.update_magnetometer(mag_data);
        }
    }
}
```

**In `server/static/nettest.html`:**
```javascript
orientationListener = (event) => {
    on_orientation(
        event.alpha,           // Pass null as-is
        event.beta,            // Pass null as-is
        event.gamma,           // Pass null as-is
        event.absolute || false
    );
};

magnetometerListener = (event) => {
    if (on_magnetometer && event.alpha !== null) {
        on_magnetometer(
            event.alpha,       // Pass actual value (not null due to check)
            event.beta,
            event.gamma,
            true
        );
    }
};
```

### Option 2: Keep current signatures but add null checks in JavaScript

If changing WASM signatures is problematic, add null checks in JavaScript:

```javascript
orientationListener = (event) => {
    // Only call if we have at least one valid value
    if (event.alpha !== null || event.beta !== null || event.gamma !== null) {
        on_orientation(
            event.alpha ?? 0,
            event.beta ?? 0,
            event.gamma ?? 0,
            event.absolute || false
        );
    }
};
```

However, this still loses the distinction between 0 and null inside WASM.

## Related Issues
- Issue 022 (resolved): on_magnetometer signature mismatch - partially addressed but didn't fix the Optional parameter issue

---
*Created: 2026-02-04*
