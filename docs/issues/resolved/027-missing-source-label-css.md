# Issue 027: Missing CSS Styles for Recording Source Labels

## Summary
The CSS styles for `.source-label`, `.source-camera`, `.source-screen`, and `.source-combined` classes are missing from `nettest.html`, but these classes will be used in the recordings list to visually distinguish between camera-only, screen-only, and combined recordings.

## Location
- File: `server/static/nettest.html`
- Section: `<style>` block (around line 7-900)
- Affects: Recordings list rendering in Issue 026 implementation

## Current Behavior
The CSS does not include styling for source type labels that distinguish recording types. When recordings are displayed (after Issue 026 is resolved), the source labels won't have any visual styling.

## Expected Behavior
Source labels should have colored backgrounds to make it easy to identify the recording type at a glance:
- Camera recordings: Green badge
- Screen recordings: Orange badge  
- Combined (Screen + Camera PiP) recordings: Purple badge

Reference implementation from `tmp/camera-standalone-for-cross-check/camera-tracker.html` (lines ~78-87):
```css
.source-label {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 10px;
    margin-left: 5px;
}
.source-camera { background: #34C759; }
.source-screen { background: #FF9500; }
.source-combined { background: #AF52DE; }
```

## Impact
**Low** - This is a visual/UX enhancement. The recordings list will work without these styles, but the user experience will be degraded as users won't be able to quickly distinguish recording types.

## Suggested Implementation

Add the following CSS to the `<style>` block in `server/static/nettest.html` (around line 830, near other `.recording-*` styles):

```css
.source-label {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 500;
    margin-left: 5px;
    color: white;
}

.source-camera { 
    background: #34C759;
}

.source-screen { 
    background: #FF9500;
}

.source-combined { 
    background: #AF52DE;
}
```

## Related Issues
- Issue 026: Missing recordings list implementation - will use these CSS classes

## Resolution

**Resolved: 2026-02-05**

Added CSS styles for recording source type labels to visually distinguish between camera, screen, and combined recordings.

### Changes Made:

**In `server/static/nettest.html` (lines ~852-869)**:
- Added `.source-label` base styles:
  - Inline-block display with padding and border radius
  - Small font size (10px) with medium weight (500)
  - White text color on colored background
  - 5px left margin for spacing from recording ID

- Added source type color styles:
  - `.source-camera`: Green background (#34C759) for camera-only recordings
  - `.source-screen`: Orange background (#FF9500) for screen-only recordings
  - `.source-combined`: Purple background (#AF52DE) for combined (screen + PiP) recordings

### Verification:
- Styles match the reference implementation from `tmp/camera-standalone-for-cross-check/camera-tracker.html`
- Color scheme follows iOS system colors for consistency
- Styles are used by the recordings list implementation (Issue 026)

---
*Created: 2026-02-04*
