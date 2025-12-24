# Dashboard Cleanup Button Design

## Overview

Add a cleanup button to each client entry in the dashboard that allows immediate removal of that client and all its descendants from the server state.

## Requirements

- Add "Cleanup" button to each row in the dashboard table
- Clicking button immediately removes client from server state (no confirmation)
- Cascade delete: automatically remove all child clients (parent_id matches)
- No graceful WebRTC shutdown - just force remove from HashMap
- Existing WebSocket updates will reflect the removal automatically

## Frontend Changes

### HTML (dashboard.html)

**Table Header:**
- Add new `<th>Actions</th>` column after "Current Seq"
- Update "No clients connected" colspan from 9 to 10

**Styling:**
- Add CSS for cleanup button:
  - Small red button
  - Text: "Cleanup" or "×" symbol
  - Class: `.cleanup-btn`
  - Should be compact to fit in table row

### JavaScript (dashboard.js)

**updateClientsTable() modifications:**
- Add new `<td>` with cleanup button in each row
- Button onclick calls `cleanupClient(client.id)`

**New cleanupClient() function:**
- Makes DELETE request to `/api/clients/{clientId}`
- Error handling: console.log errors, no user alerts
- No visual feedback needed (WebSocket will update table)

## Backend Changes

### New API Endpoint

**Route:** `DELETE /api/clients/:id`

**Handler:** `cleanup_client_handler()`
- Extract `State(AppState)` and `Path<String>` (client ID)
- Lock clients HashMap (write)
- Find all descendants recursively (parent_id chain)
- Remove target and all descendants from HashMap
- Return list of removed client IDs

**Response Format:**
- Success: 200 OK `{"removed": ["id1", "id2", ...]}`
- Not found: 404 `{"error": "Client not found"}`

**Cascade Delete Algorithm:**
1. Start with target client ID
2. Find all clients where parent_id == target ID
3. For each child, recursively find their children
4. Collect all IDs in deletion list
5. Remove all at once from HashMap

### Code Organization

- Add handler in new file `server/src/cleanup.rs` or existing routes file
- Register route in `main.rs` router
- Import necessary types: AppState, Path, Json

## Implementation Notes

- No WebRTC connection cleanup needed (just drop from state)
- Existing WebSocket updates handle UI refresh
- No race condition concerns (write lock protects HashMap)
- Recursive deletion ensures no orphaned clients remain
