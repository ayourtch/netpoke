# 053 - Axum Route Parameter Syntax Panic

## Summary

Server panics on startup because route `/admin/api/sessions/:session_id` uses the old axum 0.6 `:param` syntax, but the project uses axum 0.8 which requires `{param}` syntax.

## Location

- **File**: `server/src/main.rs`, line 166
- **Route**: `/admin/api/sessions/:session_id`

## Current Behavior

Server panics at startup with:
```
Path segments must not start with `:`. For capture groups, use `{capture}`.
```

## Expected Behavior

Server starts successfully with the route properly capturing the `session_id` path parameter.

## Impact

Server cannot start at all — complete service outage.

## Root Cause Analysis

The route was added using the old axum 0.6 colon-prefix syntax (`:session_id`) but the project depends on axum 0.8 which uses brace syntax (`{session_id}`).

## Suggested Implementation

Change line 166 of `server/src/main.rs` from:
```rust
.route("/admin/api/sessions/:session_id", get(analyst_api::get_session))
```
to:
```rust
.route("/admin/api/sessions/{session_id}", get(analyst_api::get_session))
```

## Resolution

Fixed by updating the route parameter syntax from `:session_id` to `{session_id}`.

### Files Modified
- `server/src/main.rs`: Updated route path parameter syntax on line 166
