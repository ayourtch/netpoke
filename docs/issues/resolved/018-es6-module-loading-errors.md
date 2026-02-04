# Issue 018: ES6 Module Syntax Errors in Recorder JavaScript Files

## Summary
The recorder JavaScript files (`indexed_db.js` and `media_recorder.js`) use ES6 module syntax with `export` statements, but they're loaded as regular scripts in the HTML instead of as ES6 modules. This causes "Unexpected keyword 'export'" syntax errors in the browser.

## Location
- Files: `server/static/lib/recorder/indexed_db.js`, `server/static/lib/recorder/media_recorder.js`
- HTML: `server/static/nettest.html` (lines 1110-1111)

## Current Behavior
Browser console errors:
```
[Error] SyntaxError: Unexpected keyword 'export'
	(anonymous function) (indexed_db.js:3)
[Error] SyntaxError: Unexpected keyword 'export'
	(anonymous function) (media_recorder.js:4)
```

The HTML loads these files as regular scripts:
```html
<script src="/static/lib/recorder/indexed_db.js"></script>
<script src="/static/lib/recorder/media_recorder.js"></script>
```

But the files use ES6 module exports:
```javascript
// indexed_db.js
export async function openDb() { ... }

// media_recorder.js
export function createMediaRecorder(stream) { ... }
```

## Expected Behavior
These files are actually imported as ES6 modules by the WASM module via:
```rust
// client/src/recorder/storage.rs
#[wasm_bindgen(module = "/static/lib/recorder/indexed_db.js")]
extern "C" {
    #[wasm_bindgen(catch)]
    pub async fn openDb() -> Result<JsValue, JsValue>;
    // ...
}
```

The WASM bindgen handles module imports correctly, but the HTML shouldn't try to load them as scripts at all since they're only used by WASM.

## Impact
- **Priority: Low**
- Console errors are shown but don't break functionality
- The files work fine when imported by WASM modules
- Confusing for developers debugging
- Unnecessary HTTP requests for files that won't be used

## Suggested Implementation

**Option 1: Remove the script tags** (RECOMMENDED)
These files are only used by WASM modules, not by HTML JavaScript. Remove the script tags:
```html
<!-- DELETE these lines from nettest.html -->
<script src="/static/lib/recorder/indexed_db.js"></script>
<script src="/static/lib/recorder/media_recorder.js"></script>
```

**Option 2: Convert to ES6 module script tags** (if HTML needs them)
If the HTML actually needs these files, load them as modules:
```html
<script type="module" src="/static/lib/recorder/indexed_db.js"></script>
<script type="module" src="/static/lib/recorder/media_recorder.js"></script>
```

**Option 3: Keep scripts but remove exports** (not recommended)
Create dual versions - one for WASM import and one for HTML, but this adds maintenance burden.

## Resolution Steps
1. Verify that no HTML JavaScript code directly calls functions from these files
2. If confirmed, remove the `<script>` tags from nettest.html
3. The WASM module will continue to import them correctly via `#[wasm_bindgen(module = "...")]`

---
*Created: 2026-02-04*

*Resolved: 2026-02-04*

## Resolution

**Status**: Already resolved in current codebase.

### What Was Found
The ES6 module script tags for `indexed_db.js` and `media_recorder.js` mentioned in the issue are not present in the current version of `server/static/nettest.html`.

A search for these script tags yielded no results:
```bash
grep -n "indexed_db.js\|media_recorder.js" server/static/nettest.html
# No results
```

### Analysis
These script tags were likely removed in a previous update. The ES6 modules are correctly imported only by the WASM module using `#[wasm_bindgen(module = "...")]` declarations, which is the proper approach.

### Files Checked
- `server/static/nettest.html` - Confirmed script tags are not present
- `client/src/recorder/storage.rs` - Confirmed WASM module imports work correctly

### Conclusion
No action needed. The issue was already resolved by removing the redundant script tags.
