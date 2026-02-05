# NetPoke Agent Session Context

This file captures the knowledge and decisions made during the initial setup and development of NetPoke (formerly WiFi-Verify).

## Issue Fixing Process

**IMPORTANT**: When a user asks to fix something (bugs, issues, problems, or defects), follow this structured troubleshooting process:

### 1. Find and Document the Issue

Before making any code changes, create an issue file following the process in `prompts/find-issues.md`:

1. Investigate and understand the problem thoroughly
2. Identify the root cause
3. Create an issue file in `docs/issues/open/` using the next available issue number
4. Use the naming convention: `NNN-short-description.md` (e.g., `033-broken-traceroute-response.md`)
5. Document in the issue file:
   - **Summary**: Brief description of the issue
   - **Location**: File paths, function names, line numbers
   - **Current Behavior**: What's happening (the bug)
   - **Expected Behavior**: What should happen
   - **Impact**: How this affects users or the system
   - **Root Cause Analysis**: Why this is happening
   - **Suggested Implementation**: Proposed fix plan with specific steps

### 2. Fix the Issue

Once the issue is documented, implement the fix following `prompts/fix-issues.md`:

1. Verify the issue still exists before fixing
2. Make minimal, targeted changes to resolve the issue
3. Build and test to verify the fix works
4. Update the issue file with a **Resolution** section documenting:
   - What changes were made
   - Which files were modified
   - How it was verified

### 3. Move the Resolved Issue

After the fix is complete:

```bash
git mv docs/issues/open/NNN-description.md docs/issues/resolved/
```

### Why This Process Matters

- **Documentation**: Creates a historical record of issues and their resolutions
- **Root Cause Analysis**: Ensures problems are understood before being "fixed"
- **Knowledge Transfer**: Helps future agents understand what went wrong and why
- **Prevents Regression**: Documented issues make it easier to catch if they return

### Reference Files

- `prompts/find-issues.md`: Detailed guidance for identifying and documenting issues
- `prompts/fix-issues.md`: Detailed guidance for implementing fixes
- `docs/issues/README.md`: Complete issue tracking process and file format

## Project Overview

**NetPoke** is a browser-based network measurement and survey platform that measures end-to-end application-layer network performance. Unlike traditional WiFi survey tools that focus on radio frequency (RF) metrics, NetPoke measures what applications actually experience: latency, jitter, packet loss, path characteristics, and throughput.

### Key Features
- Browser-based traceroute using modified WebRTC
- Per-packet UDP socket options (TTL, TOS, DF bit) for traceroute
- Packet capture service for survey-specific pcap downloads
- DTLS keylog service for storing encryption keys per survey session
- Camera/sensor capture integration for surveys
- Organization/project/magic-key hierarchy for access control
- Dual-stack (IPv4/IPv6) support

### Technical Architecture
- **Server**: Rust with WebRTC, handles signaling and packet tracking
- **Client**: Rust compiled to WASM, runs in browser
- **Auth**: `netpoke-auth` crate with "Project Raindrops" branding
- **Vendored WebRTC**: 6 crates (webrtc, webrtc-util, webrtc-data, webrtc-sctp, dtls, webrtc-ice) modified for UDP socket options

## Session History

### Initial Issues Discovered (2026-01-08)

1. **Cargo fmt commit `8b62976` accidentally removed working code**
   - Stripped out `capture_service` and `keylog_service` fields from `AppState` and `ClientSession` structs
   - Removed `SessionRegistry` and per-session packet filtering from `packet_capture.rs`
   - Removed `notify_survey_session_id_js()` function from client WASM code
   - Removed DTLS keylog service initialization in `main.rs`

2. **macOS build broken**
   - Vendored `webrtc-sctp` crate had Linux-only code not properly guarded with `#[cfg(target_os = "linux")]`
   - Per-packet socket options only work on Linux (by design)

3. **Resolution**
   - Reset `main` branch to commit `5f394ba` (last known-good working state before bad fmt commit)
   - Fixed macOS build by adding `#[cfg(target_os = "linux")]` guards:
     - `vendored/webrtc-sctp/src/association/association_internal.rs`: Wrap `udp_send_options` access
     - `vendored/webrtc-sctp/src/stream/mod.rs`: Platform-specific `prepare_write()` calls

### Commits Applied to Main

1. **`8c6ccf3`** - fix: add cfg guards for Linux-only UDP socket options in webrtc-sctp
   - Fixes macOS build while preserving per-packet socket options on Linux

2. **`5d9048c`** - docs: add comprehensive product documentation and design style guide
   - Product overview, technical architecture, roadmap
   - Organization model, competitive positioning
   - Design style guide (colors, typography, components)

3. **`9cefa52`** - style: apply cargo fmt formatting across codebase
   - 50 files reformatted
   - Verified capture_service and keylog_service remained intact

4. **`e8b4d6c`** - refactor: rename wifi-verify to netpoke
   - Package names: `netpoke-auth`, `netpoke-client`, `netpoke-server`
   - Product name: NetPoke (netpoke.com)
   - Preserved "Project Raindrops" branding on auth/login pages
   - Updated all documentation and code references

## Important Files and Their Functions

### Server Core

**`server/src/state.rs`**
- `AppState`: Contains `capture_service` and `keylog_service` (both `Option<Arc<...>>`)
- `ClientSession`: Per-client session with `capture_service` and `keylog_service` fields
- `InstrumentedRwLock`: Custom RwLock wrapper for debugging lock acquisition locations

**`server/src/packet_capture.rs`**
- `SessionRegistry`: Maps client addresses to survey session IDs for per-session packet filtering
- `PacketCaptureService`: Manages pcap files per survey session
- Key methods: `register_session()`, `write_packet()`, `get_pcap_for_session()`
- Per-session filtering uses `SessionRegistry::get_session_id(client_addr)`

**`server/src/dtls_keylog.rs`**
- `DtlsKeylogService`: Stores DTLS keylog entries per survey session for Wireshark decryption
- `DtlsKeylogEntry`: Contains `client_random`, `master_secret`, `label`, `survey_session_id`
- Methods: `add_entry()`, `get_entries_for_session()`, `get_sslkeylogfile_for_session()`

**`server/src/main.rs`**
- Initializes `PacketCaptureService` and `DtlsKeylogService`
- Sets `capture_service` and `keylog_service` on `AppState`
- Services passed to data channel handlers via `data_channel.rs`

**`server/src/data_channels.rs`**
- Data channel message handlers that register with capture/keylog services
- When survey session ID is received, registers session with `SessionRegistry`
- Passes services to new data channels

**`server/src/packet_tracker.rs`**
- `PacketTracker`: Tracks sent packets and matches ICMP errors
- `TrackedPacket`: Contains `send_options` (TTL, TOS, DF), `conn_id`, `seq`
- Per-session filtering: `drain_events_for_conn_id()` (new method, `drain_events()` is deprecated)

### Client

**`client/src/lib.rs`**
- `notify_survey_session_id_js()`: Sends survey session ID to JavaScript for camera tracking
- This function was accidentally removed by cargo fmt but restored

**`client/src/webrtc.rs`**
- WebRTC connection management
- Handles data channels and signaling

### Auth

**`auth/src/views.rs`**
- Login and access denied pages
- **IMPORTANT**: Must preserve "Project Raindrops" branding (do NOT change to NetPoke)

### Vendored Crates (Linux-Only Modifications)

**`vendored/webrtc-sctp/src/association/association_internal.rs`**
- `bundle_data_chunks_into_packets()`: Access to `c.udp_send_options` wrapped in `#[cfg(target_os = "linux")]`
- Per-packet UDP options (TTL, TOS, DF) only used on Linux

**`vendored/webrtc-sctp/src/stream/mod.rs`**
- `prepare_write()`: Takes `udp_send_options` arg on Linux, 2 args on other platforms
- `write_sctp()`: Calls `prepare_write()` with platform-specific args using cfg guards
- `write_sctp_with_options()`: Linux-only method for writing with UDP options

**`vendored/webrtc-util/src/conn/mod.rs`**
- `send_with_options()` and `send_to_with_options()`: Default implementations log warnings if not forwarded
- Logs indicate missing implementation in call chain (helpful debugging)

## Per-Packet Socket Options (Linux-Only)

### Why Linux-Only?
- macOS doesn't support per-packet TTL/TOS via `sendmsg()` control messages
- macOS only supports socket-level `setsockopt()` which has race conditions in multi-threaded environment
- Decision: Keep this Linux-only feature as-is

### Implementation
1. Client (WASM): Sets UDP options on WebRTC packets
2. Vendored webrtc-sctp: Forwards options through SCTP layer (Linux-only code paths)
3. Vendored webrtc-util: UDP connection wrapper with `send_with_options()` methods
4. Vendored dtls/webrtc: Forward options through DTLS/WebRTC layers
5. Server: Receives packets with original socket options intact

### Code Pattern
```rust
#[cfg(target_os = "linux")]
{
    // Linux-specific code accessing udp_send_options
}
// Cross-platform code continues
```

## Organization Model

From `docs/plans/organization-model.md`:

- **Organization**: Top-level entity for company or department
- **Project**: Collection of surveys for a client or network
- **Magic Key**: Short code for access without OAuth (e.g., "ABC123")
- **Survey Session**: Single test run with associated pcap and keylog data

Access control:
- OAuth users have organization/project access
- Magic keys grant temporary project access

## Important Branding Decisions

- **Product Name**: NetPoke (netpoke.com)
- **Auth/Login**: "Project Raindrops" - KEEP THIS, do not change to NetPoke
- **Repository**: netpoke (GitHub: git@github.com:ayourtch/netpoke.git)
- **Domain**: netpoke.com
- **Environment Variables**: `NETPOKE_*` prefix (e.g., `NETPOKE_SERVER_ENABLE_HTTPS`)

## Build Notes

### macOS Build
- Requires `#[cfg(target_os = "linux")]` guards around Linux-only UDP socket option code
- Per-packet TTL/TOS/DF features are not available on macOS

### Build Commands
```bash
cargo build              # Debug build
cargo build --release   # Release build
cargo fmt               # Format code
```

### Important: After cargo fmt
- Verify `capture_service` and `keylog_service` fields still exist in:
  - `server/src/state.rs` (AppState and ClientSession)
- Verify `SessionRegistry` exists in `server/src/packet_capture.rs`
- Verify `notify_survey_session_id_js()` exists in `client/src/lib.rs`
- Verify services are initialized in `server/src/main.rs`

## Common Patterns

### Adding New Features

1. **Service Initialization** (main.rs):
   ```rust
   let my_service = Arc::new(MyService::new());
   ```

2. **Adding to AppState** (state.rs):
   ```rust
   pub struct AppState {
       pub my_service: Option<Arc<MyService>>,
   }
   ```

3. **Per-Session Data**:
   - Use `SessionRegistry` in `PacketCaptureService` as pattern
   - Register sessions in data channel handlers when survey session ID received
   - Store data keyed by survey session ID

### Debugging Lock Issues
- Use `InstrumentedRwLock::read("label")` and `write("label")` with descriptive labels
- Helps identify deadlock locations

## Documentation Structure

- `docs/README.md`: Main documentation index
- `docs/plans/`: Product plans (roadmap, architecture, organization model)
- `docs/history/`: Historical fix descriptions (why certain changes were made)
- `IMPLEMENTATION_NOTES.md`: Implementation notes and TODOs
- `TRACEROUTE_INVESTIGATION_SUMMARY.md`: Traceroute debugging history

## Common Gotchas

1. **Cargo fmt removing code**: Always verify critical functions/fields after running cargo fmt
2. **macOS build failures**: Check for missing `#[cfg(target_os = "linux")]` guards in vendored crates
3. **Per-session filtering**: Use `SessionRegistry` pattern, don't bypass session tracking
4. **"Project Raindrops" branding**: Never change to NetPoke - it's the auth system name

## Future Agents - Quick Start

When working on NetPoke:

1. **Current main branch commit**: `e8b4d6c` (as of 2026-01-08)
2. **Key files to check**: `server/src/state.rs`, `server/src/packet_capture.rs`, `server/src/dtls_keylog.rs`
3. **Critical features**: Per-session packet capture, DTLS keylog, per-packet UDP options (Linux-only)
4. **Platform consideration**: Linux has per-packet socket options, macOS does not
5. **Branding**: NetPoke for product, Project Raindrops for auth/login

## Session Summary

This session accomplished:
1. Fixed accidental removal of capture/keylog services by cargo fmt
2. Fixed macOS build by adding cfg guards for Linux-only code
3. Applied comprehensive documentation and design style guide
4. Applied cargo fmt (verified no feature regression)
5. Renamed product from WiFi-Verify to NetPoke
6. Pushed to new netpoke repository (git@github.com:ayourtch/netpoke.git)

All critical features are verified working on this branch.
