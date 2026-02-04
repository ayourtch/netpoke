# Cache Busting Guide

## For Developers

When you rebuild the WASM package and want to force browsers to reload fresh code:

1. Build normally: `wasm-pack build --target web --out-dir pkg`

2. Add cache busting to WASM binary (Safari caches .wasm files aggressively):
   ```bash
   sed -i.bak "s|'camera_wasm_bg.wasm'|'camera_wasm_bg.wasm?v=NEW_VERSION'|g" pkg/camera_wasm.js
   ```
   Replace `NEW_VERSION` with the new version number (e.g., 16)

3. Open `index.html` and increment the `APP_VERSION` number to match:

```javascript
// Change this:
const APP_VERSION = 1;

// To this:
const APP_VERSION = 2;
```

4. Deploy both `index.html` and the `pkg/` directory

That's it! Browsers will see the new version number and load fresh WASM files.

## For Mobile Safari Users

If you see errors after an update even with the version incremented:

1. Tap the refresh button in Safari's address bar
2. Hold until a menu appears
3. Select "Request Desktop Website"
4. This forces a complete cache clear

Alternatively:
- Settings → Safari → Clear History and Website Data
- Then reload the page

## Why This Approach?

- **Simple**: Just increment a number, no build scripts needed
- **Maintainable**: Works with standard `wasm-pack build`
- **Effective**: Query parameters prevent cache reuse
- **Manual control**: You decide when to force cache clear

## Meta Tags

The HTML includes cache-control meta tags:
```html
<meta http-equiv="Cache-Control" content="no-cache, no-store, must-revalidate">
<meta http-equiv="Pragma" content="no-cache">
<meta http-equiv="Expires" content="0">
```

These tell browsers not to cache aggressively, but mobile Safari sometimes ignores them. That's why we use the version number.
