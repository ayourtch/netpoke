# Vendored webrtc-util Version Information

This directory contains a vendored copy of the `webrtc-util` crate with custom modifications for the wifi-verify project.

## Version Information

- **Crate Name**: webrtc-util
- **Version**: 0.12.0
- **Original Repository**: https://github.com/webrtc-rs/webrtc
- **Path in Repository**: util/
- **Commit SHA**: a1f8f1919235d8452835852e018efd654f2f8366
- **Crates.io Release**: https://crates.io/crates/webrtc-util/0.12.0
- **Vendored Date**: 2025-12-26

## Purpose

This crate is vendored to add per-packet UDP socket options support (TTL, TOS/DSCP, DF bit) using Linux's `sendmsg()` system call with control messages. These modifications enable WebRTC data channels to control UDP packet attributes at per-message granularity.

## Modified Files

The following files have been modified from the original crate:

1. **Cargo.toml**
   - Added: `libc = "0.2"` dependency for Linux-specific socket operations
   - Comment: "Added for wifi-verify: UDP socket options support"

2. **src/conn/conn_udp.rs**
   - Added: `UdpSendOptions` struct for per-message options
   - Added: Thread-local storage for options passing
   - Added: `set_send_options()` and `get_current_send_options()` functions
   - Added: `send_to_with_options()` function using Linux `sendmsg()`
   - Added: `sendmsg_with_options()` implementation with control messages
   - Modified: `Conn::send_to()` to check for options and use custom path
   - Lines added: ~195 lines (62-256)

3. **src/conn/mod.rs**
   - Added: Re-exports of `UdpSendOptions` and `set_send_options`
   - Comment: "Re-export UDP socket options support (added for wifi-verify)"

4. **src/lib.rs**
   - Added: Public re-exports at crate level
   - Comment: "Added for wifi-verify"

## How to Update

See [Updating Process](../../docs/UDP_PACKET_OPTIONS.md#updating-vendored-webrtc-util) for detailed instructions.

Quick update:
```bash
# Update to new version (adjust version number as needed)
./scripts/refresh-vendored.sh

# Verify changes
cargo check --all

# Test
cargo test --all
```

## Modification Strategy

The modifications use conditional compilation (`#[cfg(target_os = "linux")]`) to ensure:
- Linux systems get full per-packet control via `sendmsg()`
- Other platforms gracefully fall back to standard `send_to()`
- No API changes to the WebRTC stack itself
- Options passed via thread-local storage to avoid modifying upstream APIs

## Maintenance Notes

When updating to a newer version of webrtc-util:
1. The modifications are substantial (~200 lines) and cannot be easily rebased
2. You may need to manually re-apply the changes
3. The original unmodified version can be downloaded from crates.io
4. Compare using: `diff -ur <original> <modified>` to see all changes
5. Look for the marker comments "wifi-verify" to identify all modified sections
