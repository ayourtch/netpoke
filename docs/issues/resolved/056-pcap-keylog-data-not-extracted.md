# Issue 056: PCAP and Keylog Data Not Extracted for Sessions

## Summary
The admin panel's `has_pcap` and `has_keylog` flags always show as false because the `pcap_path` and `keylog_path` database columns are never populated. The PCAP and keylog data IS available through in-memory services (PacketCaptureService and DtlsKeylogService) but the analyst API only checks the database columns.

## Location
- `server/src/analyst_api.rs` - `list_sessions()` and `get_session()` check `pcap_path`/`keylog_path` DB columns
- `server/src/session_manager.rs` - `update_session_files()` exists but is never called in production
- `server/static/admin/surveys.html` - Shows badges based on `has_pcap`/`has_keylog` flags

## Current Behavior
- `has_pcap` is determined by `row.get::<_, Option<String>>(4)?.is_some()` which checks `pcap_path` in DB
- `has_keylog` is determined by `row.get::<_, Option<String>>(5)?.is_some()` which checks `keylog_path` in DB
- Neither column is ever populated because `update_session_files()` is only called in tests
- The admin panel always shows "No PCAP" and "No Keylog" even when data exists in memory

## Expected Behavior
The analyst API should check the in-memory capture and keylog services for data availability, and the admin panel should provide download links to the existing API endpoints.

## Impact
- **Priority**: High
- Users cannot tell if PCAP/keylog data is available for a session
- No download links are shown in the admin panel for PCAP/keylog data

## Suggested Implementation
1. Add `capture_service` and `keylog_service` to `AnalystState`
2. Add `has_keylogs_for_session()` and `has_packets_for_session_id()` methods to services
3. Check in-memory services in the analyst API handlers
4. Add download links in the admin panel pointing to existing API endpoints

## Created
2026-02-06

## Resolution
1. Added `has_keylogs_for_session()` to `DtlsKeylogService` and `has_session_registered()` to `PacketCaptureService` for efficient availability checks
2. Added `capture_service` and `keylog_service` to `AnalystState`
3. Updated `list_sessions` and `get_session` API handlers to check in-memory services for PCAP/keylog availability
4. Added `has_pcap` and `has_keylog` fields to `SessionDetails` response
5. Added download links for PCAP and keylog data in the admin panel

### Files Modified
- `server/src/dtls_keylog.rs` - Added `has_keylogs_for_session()` and `has_session()` methods
- `server/src/packet_capture.rs` - Added `registered_session_ids` HashSet and `has_session_registered()` method
- `server/src/analyst_api.rs` - Updated `AnalystState`, `list_sessions`, `get_session`
- `server/src/main.rs` - Pass services to AnalystState, clone keylog_service before move
- `server/static/admin/surveys.html` - Added PCAP/keylog download links

## Resolved
2026-02-06
