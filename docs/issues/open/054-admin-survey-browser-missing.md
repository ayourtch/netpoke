# 054 - Admin Survey Browser Not Visible / Missing Access Control

## Summary

Upload functionality works, but uploaded survey results are not visible in the admin panel. The design spec (`docs/plans/2026-02-05-survey-upload-feature-design.md`) specifies an analyst UI at `/admin/surveys` and access control via `[analyst_access]` configuration, but neither has been implemented.

## Location

- `server/src/analyst_api.rs` - API endpoints exist but lack access control
- `server/src/config.rs` - Missing `analyst_access` configuration
- `server/static/admin/` - Directory does not exist (no surveys.html page)
- `server/src/main.rs` - No route for serving the admin surveys page
- `server/static/dashboard.html` - No navigation link to survey browser

## Current Behavior

1. Analyst API endpoints (`/admin/api/sessions`, `/admin/api/magic-keys`, `/admin/api/sessions/{id}`) exist and are protected by `require_auth` middleware
2. No HTML page exists to view survey data - only raw API endpoints
3. No `analyst_access` configuration exists to map users to magic keys
4. No access control filtering on the analyst API - any authenticated user can see all data
5. Dashboard has no link to view uploaded survey results

## Expected Behavior

1. An admin survey browser page at `/admin/surveys` shows uploaded survey data
2. `[analyst_access]` configuration maps usernames to magic keys they can view
3. The "admin" user can view ALL magic keys and their measurements (wildcard access)
4. The analyst API filters results based on the logged-in user's allowed magic keys
5. Dashboard includes navigation to the survey browser

## Impact

Users who upload survey data cannot view the results through the admin panel. The admin user has no way to browse survey sessions, recordings, or metrics through the web interface.

## Root Cause Analysis

Phase 3 of the implementation plan (Analyst UI) was not completed. The backend API exists but the frontend page and access control configuration were not built.

## Suggested Implementation

1. Add `analyst_access` field to `Config` struct (HashMap<String, Vec<String>>)
2. Default to `"admin" = ["*"]` for wildcard access
3. Pass analyst_access config to `AnalystState`
4. Add user identity extraction to analyst API handlers (via `SessionData` in request extensions)
5. Filter `list_magic_keys` and `list_sessions` based on user's allowed keys
6. Create `server/static/admin/surveys.html` with survey browsing UI
7. Add route for the surveys page in `main.rs`
8. Add navigation link from dashboard to surveys page
