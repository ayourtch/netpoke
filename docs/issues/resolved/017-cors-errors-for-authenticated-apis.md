# Issue 017: CORS Errors for Authenticated API Endpoints

## Summary
The browser console shows CORS (Cross-Origin Resource Sharing) errors when accessing `/api/capture/stats` and `/api/tracing/stats` endpoints. These errors occur because the endpoints require authentication (hybrid auth) but the fetch requests may not include proper credentials or CORS headers.

## Location
- File: `server/static/nettest.html`
- Functions: `checkCaptureStatus()` (line 2443), `checkTracingStatus()` (line 2477)
- Server Routes: `server/src/main.rs` (lines 99, 110)

## Current Behavior
Browser console errors:
```
[Error] Fetch API cannot load https://sandbox.netpoke.com/api/capture/stats due to access control checks.
[Error] Fetch API cannot load https://sandbox.netpoke.com/api/tracing/stats due to access control checks.
[Error] Capture status check failed: – TypeError: Load failed
[Error] Tracing status check failed: – TypeError: Load failed
```

The JavaScript makes fetch requests without credentials:
```javascript
const response = await fetch('/api/capture/stats');
```

These routes are protected with hybrid authentication middleware in the server:
```rust
let hybrid_capture_session = capture_session_routes.route_layer(
    middleware::from_fn_with_state(
        auth_state.clone(),
        survey_middleware::require_auth_or_survey_session,
    )
);
```

## Expected Behavior
The fetch requests should either:
1. Include credentials (`credentials: 'include'`) to pass authentication cookies/tokens
2. Be made from an authenticated context
3. Handle authentication errors gracefully with appropriate user feedback

## Impact
- **Priority: Medium**
- Users see error messages in browser console
- Capture and tracing status indicators show "Error checking status"
- Download buttons may be incorrectly disabled
- Users don't know if capture/tracing is working
- Not a blocker for core functionality but creates confusion

## Suggested Implementation

**Option 1: Add credentials to fetch requests**
```javascript
async function checkCaptureStatus() {
    try {
        const response = await fetch('/api/capture/stats', {
            credentials: 'include',  // Include cookies/auth
            mode: 'same-origin'      // Ensure same-origin
        });
        // ... rest of code
    } catch (e) {
        // Handle auth errors gracefully
    }
}
```

**Option 2: Check authentication first**
```javascript
async function checkCaptureStatus() {
    // First check if user is authenticated
    const authResponse = await fetch('/api/auth/status', {
        credentials: 'include'
    });
    
    if (!authResponse.ok) {
        captureStatusEl.textContent = 'Authentication required';
        return;
    }
    
    // Then check capture status
    const response = await fetch('/api/capture/stats', {
        credentials: 'include'
    });
    // ... rest of code
}
```

**Option 3: Make these endpoints public (if appropriate)**
If capture/tracing stats don't contain sensitive information, consider moving them to public routes without authentication. This would require:
1. Creating a new router without auth middleware for these stats endpoints
2. Ensuring the stats don't leak sensitive data

**Option 4: Handle 401/403 gracefully**
```javascript
async function checkCaptureStatus() {
    try {
        const response = await fetch('/api/capture/stats', {
            credentials: 'include'
        });
        
        if (response.status === 401 || response.status === 403) {
            captureStatusEl.textContent = 'Authentication required for capture stats';
            captureStatusEl.style.color = '#ff9800';
            return;
        }
        
        if (response.ok) {
            // ... process stats
        }
    } catch (e) {
        // Network errors
        captureStatusEl.textContent = 'Unable to reach server';
    }
}
```

## Related Context
- CORS is configured in `server/src/config.rs` with `enable_cors` setting
- Authentication state is managed by `netpoke-auth` crate
- These routes use hybrid auth (either full auth OR survey session magic key)
- Same issue likely affects other authenticated API endpoints called from JavaScript

---
*Created: 2026-02-04*

*Resolved: 2026-02-04*

## Resolution

**Status**: Resolved by adding credentials to fetch requests and graceful authentication error handling.

### Changes Made

Added proper CORS credentials and error handling to both capture and tracing status check functions in `server/static/nettest.html`:

1. **Updated `checkCaptureStatus()`** (line ~2453):
   - Added `credentials: 'include'` to fetch request
   - Added `mode: 'same-origin'` for same-origin requests
   - Added specific handling for 401/403 responses showing "Authentication required" message
   - Distinguished between auth errors (orange warning) and network errors (red error)

2. **Updated `checkTracingStatus()`** (line ~2487):
   - Added `credentials: 'include'` to fetch request
   - Added `mode: 'same-origin'` for same-origin requests
   - Added specific handling for 401/403 responses showing "Authentication required" message
   - Distinguished between auth errors (orange warning) and network errors (red error)

### Files Modified
- `server/static/nettest.html` - Updated fetch calls with credentials and auth error handling

### Result
- CORS errors are eliminated when user is authenticated
- When not authenticated, users see clear "Authentication required" message instead of generic errors
- Error handling differentiates between authentication issues, server issues, and network failures
- No changes needed to server-side code - the endpoints already support hybrid authentication
