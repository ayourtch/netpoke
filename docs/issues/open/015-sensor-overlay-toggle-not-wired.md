# Issue 015: Missing Sensor Overlay Toggle Checkbox Integration

## Summary
The design document specifies a "Show Sensors Overlay" checkbox in the recording panel UI. While the HTML has such a checkbox (id="show-sensors-overlay"), there's no event listener to handle changes, and the overlay enabled state is not properly synchronized with the SensorManager.

## Location
- File: `server/static/nettest.html` - Has checkbox but no event handling
- File: `client/src/recorder/ui.rs` - Missing sensor overlay toggle setup
- File: `client/src/recorder/sensors.rs` - Has `set_overlay_enabled()` method

## Current Behavior
The HTML (around line 960) has:
```html
<label>
    <input type="checkbox" id="show-sensors-overlay" checked>
    Show Sensors Overlay
</label>
```

But there's no event listener in `ui.rs` to handle changes to this checkbox.

In `sensors.rs`, the SensorManager has:
```rust
pub fn set_overlay_enabled(&mut self, enabled: bool) {
    self.overlay_enabled = enabled;
}

pub fn is_overlay_enabled(&self) -> bool {
    self.overlay_enabled
}
```

But these are never called from the UI.

## Expected Behavior
The standalone camera exports a WASM function and sets up the listener:
```rust
// In lib.rs:
#[wasm_bindgen]
pub fn set_sensor_overlay_enabled(enabled: bool) {
    if let Ok(mut manager) = SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.set_overlay_enabled(enabled);
        }
    }
}
```

And the checkbox change should call this function to toggle the overlay visibility.

## Impact
- **Priority: Low**
- Sensor overlay checkbox has no effect
- Users cannot hide the sensor overlay during recording
- Overlay is always shown (or never shown, depending on initialization)

## Suggested Implementation
1. **Export the toggle function** in `client/src/lib.rs`:
```rust
#[wasm_bindgen]
pub fn set_sensor_overlay_enabled(enabled: bool) {
    if let Ok(mut manager) = crate::SENSOR_MANAGER.lock() {
        if let Some(mgr) = manager.as_mut() {
            mgr.set_overlay_enabled(enabled);
        }
    }
}
```

2. **Add event listener in JavaScript** (nettest.html):
```javascript
document.getElementById('show-sensors-overlay').addEventListener('change', (e) => {
    set_sensor_overlay_enabled(e.target.checked);
});
```

3. **Or handle in Rust** (`client/src/recorder/ui.rs`):
```rust
fn setup_sensor_overlay_toggle(document: &web_sys::Document) {
    if let Some(checkbox) = document.get_element_by_id("show-sensors-overlay") {
        let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
            if let Some(target) = event.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    if let Ok(mut manager) = crate::SENSOR_MANAGER.lock() {
                        if let Some(mgr) = manager.as_mut() {
                            mgr.set_overlay_enabled(input.checked());
                        }
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);

        if let Ok(element) = checkbox.dyn_into::<web_sys::HtmlElement>() {
            let _ = element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}
```

4. **Call `setup_sensor_overlay_toggle()` from `init_recorder_panel()`**.

---
*Created: 2026-02-04*
