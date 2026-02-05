# Issue 030: PiP and Chart Position Buttons Missing IDs

## Summary
The PiP and chart position buttons in `nettest.html` are missing the required ID attributes that the WASM code expects, so clicking these buttons has no effect. The WASM code looks for elements with specific IDs but the HTML only has `data-position` attributes.

## Location
- **HTML**: `server/static/nettest.html` (lines ~928-932 for PiP, ~952-956 for chart)
- **WASM**: `client/src/recorder/ui.rs` (lines 109-112, 201-204)

## Current Behavior

**In `server/static/nettest.html`:**
```html
<!-- PiP position buttons (lines ~928-932) -->
<div class="position-selector">
    <button data-position="topleft">TL</button>
    <button data-position="topright">TR</button>
    <button data-position="bottomleft">BL</button>
    <button data-position="bottomright" class="selected">BR</button>
</div>

<!-- Chart position buttons (lines ~952-956) -->
<div class="position-selector">
    <button data-position="topleft">TL</button>
    <button data-position="topright">TR</button>
    <button data-position="bottomleft">BL</button>
    <button data-position="bottomright" class="selected">BR</button>
</div>
```

**In `client/src/recorder/ui.rs`:**
```rust
fn setup_pip_controls(document: &web_sys::Document) {
    // ...
    // PiP position buttons
    setup_pip_position_button(document, "pip-pos-tl", PipPosition::TopLeft);
    setup_pip_position_button(document, "pip-pos-tr", PipPosition::TopRight);
    setup_pip_position_button(document, "pip-pos-bl", PipPosition::BottomLeft);
    setup_pip_position_button(document, "pip-pos-br", PipPosition::BottomRight);
}

fn setup_chart_controls(document: &web_sys::Document) {
    // ...
    // Chart position buttons
    setup_chart_position_button(document, "chart-pos-tl", PipPosition::TopLeft);
    setup_chart_position_button(document, "chart-pos-tr", PipPosition::TopRight);
    setup_chart_position_button(document, "chart-pos-bl", PipPosition::BottomLeft);
    setup_chart_position_button(document, "chart-pos-br", PipPosition::BottomRight);
}

fn setup_pip_position_button(document: &web_sys::Document, id: &str, position: PipPosition) {
    if let Some(button) = document.get_element_by_id(id) {  // ← Tries to find by ID
        // Attach click handler
    }
}
```

The WASM code tries to find buttons by ID (e.g., `"pip-pos-tl"`) but the HTML buttons don't have IDs, only `data-position` attributes. Result: `get_element_by_id()` returns `None` and no event listeners are attached.

## Expected Behavior
Buttons should have IDs that match what the WASM code expects, allowing users to change the position of the camera overlay and chart overlay in recordings.

## Impact
**High** - Users cannot control where the camera and chart overlays appear in their recordings. The position selector buttons are visible but non-functional, creating a broken UI experience.

## Suggested Implementation

### Option 1: Add IDs to HTML (Recommended)

Update `server/static/nettest.html`:

```html
<!-- PiP position buttons -->
<div class="position-selector">
    <button id="pip-pos-tl" data-position="topleft">TL</button>
    <button id="pip-pos-tr" data-position="topright">TR</button>
    <button id="pip-pos-bl" data-position="bottomleft">BL</button>
    <button id="pip-pos-br" data-position="bottomright" class="selected">BR</button>
</div>

<!-- Chart position buttons -->
<div class="position-selector">
    <button id="chart-pos-tl" data-position="topleft">TL</button>
    <button id="chart-pos-tr" data-position="topright">TR</button>
    <button id="chart-pos-bl" data-position="bottomleft">BL</button>
    <button id="chart-pos-br" data-position="bottomright" class="selected">BR</button>
</div>
```

This is the minimal change that makes the existing WASM code work.

### Option 2: Update WASM to use data-position attributes

Alternatively, modify `client/src/recorder/ui.rs` to query by `data-position` attribute instead of ID:

```rust
fn setup_pip_controls(document: &web_sys::Document) {
    // ...
    // Query by selector instead of ID
    setup_pip_position_buttons_by_selector(document, "#pip-controls .position-selector button");
}

fn setup_pip_position_buttons_by_selector(document: &web_sys::Document, selector: &str) {
    let buttons = document.query_selector_all(selector).unwrap();
    for i in 0..buttons.length() {
        if let Some(button) = buttons.get(i) {
            if let Ok(element) = button.dyn_into::<web_sys::HtmlElement>() {
                if let Some(pos_attr) = element.get_attribute("data-position") {
                    let position = match pos_attr.as_str() {
                        "topleft" => PipPosition::TopLeft,
                        "topright" => PipPosition::TopRight,
                        "bottomleft" => PipPosition::BottomLeft,
                        "bottomright" => PipPosition::BottomRight,
                        _ => continue,
                    };
                    // Attach handler...
                }
            }
        }
    }
}
```

However, this is more complex and changes working WASM code. **Option 1 is recommended**.

## Related Issues
None - this is a new discovery from comparing reference vs integrated implementations.

---
*Created: 2026-02-04*
