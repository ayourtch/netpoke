# Issue 020: Missing Chart.js Source Maps

## Summary
The browser attempts to load source maps for Chart.js (`chart.umd.js.map`) but the files are missing, resulting in 404 errors. This is a development/debugging issue that doesn't affect functionality.

## Location
- File: Chart.js library (loaded from CDN or static files)
- Referenced in: `server/static/nettest.html`

## Current Behavior
Browser console warnings:
```
[Error] Source Map loading errors (x2)
[Error] Failed to load resource: the server responded with a status of 404 () (chart.umd.js.map, line 0)
```

The Chart.js library file includes a source map reference at the end:
```javascript
//# sourceMappingURL=chart.umd.js.map
```

But the `.map` file is not included in the static files.

## Expected Behavior
Either:
1. Include the source map files alongside the Chart.js library for better debugging
2. Remove the source map reference from the Chart.js file
3. Use a CDN version that includes source maps
4. Accept the warning as harmless (source maps are only useful for debugging)

## Impact
- **Priority: Very Low**
- Only affects developer console output
- No functional impact on the application
- Source maps are only useful for debugging Chart.js itself
- Most users/developers won't need Chart.js source maps

## Suggested Implementation

**Option 1: Ignore (RECOMMENDED)**
Source maps are optional and only useful if you need to debug Chart.js internals. The application works fine without them. Add to `.gitignore` or documentation that these warnings are expected.

**Option 2: Include source maps**
If Chart.js is served from static files:
```bash
# Download Chart.js with source maps
wget https://cdn.jsdelivr.net/npm/chart.js/dist/chart.umd.js.map
# Place in same directory as chart.umd.js
```

**Option 3: Use CDN with source maps**
```html
<!-- Chart.js via CDN (includes source maps) -->
<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.js"></script>
```

**Option 4: Remove source map reference**
If serving Chart.js locally, remove the last line from the file:
```javascript
// Remove this line from chart.umd.js
//# sourceMappingURL=chart.umd.js.map
```

## Resolution
Document that this warning is expected and harmless. No action required unless actively debugging Chart.js itself.

---
*Created: 2026-02-04*

*Resolved: 2026-02-04*

## Resolution

**Status**: Resolved by documentation - no code changes needed.

### Decision
This is a benign warning that does not affect functionality. Source maps are only useful for debugging Chart.js library internals, which is not needed for this project.

### Recommendation
The warning can be safely ignored. If it becomes a concern in the future, any of the suggested options in the issue can be implemented:
1. Download and include the source map files
2. Use a CDN that includes source maps
3. Remove the source map reference from the Chart.js file
4. Continue ignoring the warning

For now, documenting this as expected behavior is sufficient.
