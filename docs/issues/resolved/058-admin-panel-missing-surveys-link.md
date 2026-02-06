# Issue 058: Admin Panel Missing Surveys Link, Broken Video Playback, Missing Latency Charts

## Summary

Three related issues in the admin/survey panel:
1. The logged-in admin panel (public/index.html authenticated view) had no link to `/admin/surveys`
2. Video playback in the survey browser was broken — only played a brief moment then showed "error"
3. No way to view server+client latency stats for survey sessions

## Location

- `server/static/public/index.html` — authenticated view "Quick Actions" cards
- `server/src/analyst_api.rs` — video file serving endpoint
- `server/static/admin/surveys.html` — survey data browser UI
- `server/src/main.rs` — route registration

## Current Behavior (the bugs)

1. The "View Statistics" card showed `alert('Coming soon!')` instead of linking to `/admin/surveys`
2. The video endpoint served entire files without HTTP Range request support, causing HTML5 `<video>` elements to fail after the initial buffered data
3. The `survey_metrics` table had latency data (delay, jitter, loss) but no API endpoint or UI to view it

## Expected Behavior

1. Authenticated users should have a direct link to browse survey data
2. Videos should play fully with seek support via proper HTTP Range requests
3. Latency/jitter/loss metrics should be viewable as time-series charts per session

## Impact

- Users couldn't navigate from the landing page to survey data
- Video recordings were effectively unplayable in the browser
- Valuable latency measurement data was invisible to analysts

## Root Cause Analysis

1. **Missing link**: The authenticated view was stubbed out with a placeholder
2. **Video playback**: The `serve_recording_file` function used `tokio::fs::read()` to load the entire file and returned it as a `200 OK` without `Accept-Ranges` or `Content-Range` headers. Browsers send `Range` requests for `<video>` elements to enable streaming/seeking, and without range support they fail.
3. **Missing metrics**: No API endpoint existed to query `survey_metrics` table, and the UI had no chart component

## Resolution

### Changes Made

**`server/static/public/index.html`**:
- Changed "View Statistics" / "Coming soon!" card to "Survey Data" card linking to `/admin/surveys`

**`server/src/analyst_api.rs`**:
- Added HTTP Range request support to `serve_recording_file()` — accepts `HeaderMap`, parses `Range` header, returns `206 Partial Content` with `Content-Range` for video files
- Added `parse_byte_range()` helper function for RFC 7233 byte-range parsing
- Added `MetricEntry` struct and `get_session_metrics()` endpoint for `GET /admin/api/sessions/{session_id}/metrics`

**`server/src/main.rs`**:
- Registered new `/admin/api/sessions/{session_id}/metrics` route

**`server/static/admin/surveys.html`**:
- Added Chart.js script references (already available in static/lib/)
- Added CSS for metrics chart container
- Added "📈 Latency Chart" button in session details (shown when metrics exist)
- Added `toggleMetricsChart()` and `renderMetricsChart()` functions
- Chart shows delay p50/p99, jitter p50, and loss rate grouped by source+direction
- Color-coded per source (server c2s=blue, server s2c=orange, client c2s=green, client s2c=purple)
- Dual Y-axis: delay/jitter (ms) on left, loss (%) on right
- Pan and zoom support via chartjs-plugin-zoom

### Verification

- `cargo build` succeeds with no new warnings
- All changes are consistent with existing code patterns
