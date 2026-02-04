# Issue 014: Recorder Initialization Timing May Cause Race Condition

## Summary
The recorder is initialized via `init_recorder()` during page load, but this happens before the recording panel HTML elements may be fully rendered. This could cause event listeners to fail if elements don't exist yet.

## Location
- File: `server/static/nettest.html` - WASM initialization (lines 2553-2563)
- File: `client/src/recorder/ui.rs` - `init_recorder_panel()` (line 15+)

## Current Behavior
In `server/static/nettest.html`:
```javascript
// In the WASM init block:
const module = await import('/public/pkg/netpoke_client.js');
const { ..., init_recorder, recorder_render_frame } = module;

await init(`/public/pkg/netpoke_client_bg.wasm?v=${cacheBuster}`);
console.log('WASM module loaded successfully');

init_recorder();  // Called immediately after WASM load
console.log('Recorder initialized');
```

In `client/src/recorder/ui.rs` `init_recorder_panel()`:
```rust
pub fn init_recorder_panel() {
    let document = match web_sys::window()
        .and_then(|w| w.document())
    {
        Some(d) => d,
        None => return,
    };

    // Set up recording panel controls
    setup_mode_selection(&document);   // Gets elements by ID
    setup_pip_controls(&document);     // Gets elements by ID
    setup_chart_controls(&document);   // Gets elements by ID
    setup_recording_buttons(&document); // Gets elements by ID
    // ...
}
```

## Expected Behavior
The initialization should ensure DOM elements exist before setting up event listeners. Options:
1. Wait for DOMContentLoaded before initializing
2. Use MutationObserver to wait for elements
3. Check if elements exist and retry if not
4. Move initialization to after the HTML elements are defined

## Impact
- **Priority: Medium**
- Event listeners may not be attached if elements don't exist
- Controls may not respond to user input
- Silent failures (functions return early if elements missing)
- Inconsistent behavior depending on page load timing

## Suggested Implementation
**Option 1: Ensure WASM init happens after DOM ready**

Wrap the initialization in a DOMContentLoaded handler:
```javascript
document.addEventListener('DOMContentLoaded', async () => {
    try {
        const module = await import('/public/pkg/netpoke_client.js');
        // ... rest of initialization
        init_recorder();
    } catch (e) {
        // error handling
    }
});
```

**Option 2: Add element existence checks with warnings**

In `init_recorder_panel()`, log warnings when elements are missing:
```rust
fn setup_mode_selection(document: &web_sys::Document) {
    if let Some(radio) = document.get_element_by_id("mode-camera") {
        // set up listener
    } else {
        crate::recorder::utils::log("[Recorder] Warning: mode-camera element not found");
    }
    // ... etc
}
```

**Option 3: Retry initialization**

```javascript
async function initRecorderWithRetry(maxRetries = 5) {
    for (let i = 0; i < maxRetries; i++) {
        if (document.getElementById('mode-camera')) {
            init_recorder();
            return;
        }
        await new Promise(r => setTimeout(r, 100));
    }
    console.warn('Recorder elements not found after retries');
}
```

The best approach is Option 1 - ensuring DOM is ready before initialization.

## Resolution
Fixed in commit 9ab2ea2 (2026-02-04).

**Changes made:**
1. Wrapped WASM initialization in `server/static/nettest.html` with `DOMContentLoaded` event listener
2. The initialization code now only runs after the DOM is fully loaded
3. This ensures all HTML elements exist before `init_recorder()` tries to set up event listeners

The race condition is resolved - recorder initialization now always happens after DOM elements are available, preventing silent failures when attaching event listeners.

---
*Created: 2026-02-04*
*Resolved: 2026-02-04*
