# Vendored Crate Modifications

Modifications to vendored WebRTC crates for per-packet UDP socket options support.

## Overview

This project vendors six crates with modifications to enable per-packet UDP socket options (TTL, TOS, DF bit) through explicit API calls:

1. **webrtc v0.14.0** - Added `RTCDataChannel::send_with_options()` API
2. **webrtc-data v0.12.0** - Added `write_data_channel_with_options()` method
3. **webrtc-sctp v0.13.0** - Added options fields and propagation through SCTP layer
4. **webrtc-util v0.12.0** - Added `sendmsg()` implementation, `Conn::send_with_options()` trait method, and backtrace logging
5. **dtls v0.13.0** - Added `send_with_options()` forwarding in `DTLSConn` to propagate options through DTLS layer
6. **webrtc-ice v0.14.0** - Added `send_with_options()` support in `AgentConn` and candidate layers

## Architecture

Options flow explicitly through all layers:

```
RTCDataChannel::send_with_options(data, options)
  → DataChannel::write_data_channel_with_options(data, is_string, options)
  → Stream::write_sctp_with_options(data, ppi, options)
  → Chunks with options attached (ChunkPayloadData.udp_send_options)
  → Association::bundle_data_chunks_into_packets() extracts options
  → Packets with options attached (Packet.udp_send_options)
  → Association write loop extracts options
  → Conn::send_with_options(buf, options)
  → DTLSConn::send_with_options() forwards to underlying connection
  → Endpoint::send_with_options() forwards to next_conn
  → AgentConn::send_with_options() forwards through ICE layer
  → CandidatePair::write_with_options() 
  → CandidateBase::write_to_with_options()
  → UdpSocket::send_to_with_options()
  → UdpSocket sendmsg with control messages (TTL, TOS, DF bit)
```

## Version Information

### webrtc v0.14.0
- **Repository**: https://github.com/webrtc-rs/webrtc
- **Crates.io**: https://crates.io/crates/webrtc/0.14.0
- **Path**: vendored/webrtc/
- **Note**: Main workspace crate, manually vendored

### webrtc-data v0.12.0
- **Repository**: https://github.com/webrtc-rs/webrtc (data/ subdirectory)
- **Crates.io**: https://crates.io/crates/webrtc-data/0.12.0
- **Commit SHA**: `a1f8f1919235d8452835852e018efd654f2f8366`
- **Path in VCS**: `data`
- **Path**: vendored/webrtc-data/

### webrtc-sctp v0.13.0
- **Repository**: https://github.com/webrtc-rs/webrtc (sctp/ subdirectory)
- **Crates.io**: https://crates.io/crates/webrtc-sctp/0.13.0
- **Commit SHA**: `a1f8f1919235d8452835852e018efd654f2f8366`
- **Path in VCS**: `sctp`
- **Path**: vendored/webrtc-sctp/

### webrtc-util v0.12.0
- **Repository**: https://github.com/webrtc-rs/webrtc (util/ subdirectory)
- **Crates.io**: https://crates.io/crates/webrtc-util/0.12.0
- **Commit SHA**: `a1f8f1919235d8452835852e018efd654f2f8366`
- **Path in VCS**: `util`
- **Path**: vendored/webrtc-util/

### dtls v0.13.0
- **Repository**: https://github.com/webrtc-rs/dtls
- **Crates.io**: https://crates.io/crates/dtls/0.13.0
- **Commit SHA**: `e45b6e0906b9d30dd5c086ec1f31752ab92e5df9`
- **Path**: vendored/dtls/
- **Note**: Critical for forwarding options through DTLS layer to underlying UDP socket

### webrtc-ice v0.14.0
- **Repository**: https://github.com/webrtc-rs/ice
- **Crates.io**: https://crates.io/crates/webrtc-ice/0.14.0
- **Path**: vendored/webrtc-ice/
- **Note**: Implements `send_with_options()` in `AgentConn` and candidate layers to forward UDP options to the actual UDP socket

## Patch Files

### dtls/
1. **01-conn-mod-rs.patch** - Adds `send_with_options()` and `send_to_with_options()` forwarding to DTLSConn

### webrtc-util/
1. **01-cargo-toml.patch** - Adds libc dependency for Linux
2. **02-lib-rs.patch** - Exports UdpSendOptions type
3. **03-conn-mod-rs.patch** - Extends Conn trait with `send_with_options()` method and backtrace logging
4. **04-conn-udp-rs.patch** - Implements sendmsg() with control messages

Note: Patches for webrtc, webrtc-data, webrtc-sctp, and webrtc-ice are not included as these crates are fully vendored. See git history for modifications.

## Key Changes

### Removed Features
- ❌ Thread-local storage (`SEND_OPTIONS`)
- ❌ `set_send_options()` function
- ❌ `get_current_send_options()` function
- ❌ Backward compatibility code in `send_to()`

### Added Features
- ✅ `RTCDataChannel::send_with_options()` - Public API
- ✅ `DataChannel::write_data_channel_with_options()` - Data channel layer
- ✅ `Stream::write_sctp_with_options()` - SCTP stream layer
- ✅ `Conn::send_with_options()` - Connection trait method with backtrace logging
- ✅ Options fields in ChunkPayloadData and Packet structs
- ✅ Options extraction in Association packet bundling
- ✅ Options application in Association write loop
- ✅ `sendmsg()` with control messages for Linux
- ✅ `AgentConn::send_with_options()` - ICE agent connection layer
- ✅ `CandidatePair::write_with_options()` - ICE candidate pair layer
- ✅ `CandidateBase::write_to_with_options()` - ICE candidate base implementation
- ✅ Backtrace capture in default `send_with_options()` for debugging

## Applying Patches

To apply patches to fresh copy of webrtc-util v0.12.0:

```bash
# Download fresh crate
cargo download webrtc-util@0.12.0

# Apply patches in order
cd webrtc-util-0.12.0
for patch in ../patches/webrtc-util/*.patch; do
    patch -p1 < "$patch"
done
```

## Maintenance

When updating vendored crates:

1. Download fresh crate from crates.io
2. Extract to vendored directory
3. Try applying patches (may need adjustment)
4. If patches fail, manually apply changes
5. Regenerate patches if needed
6. Test thoroughly with `cargo check --all`

## Documentation

For complete implementation details, usage guide, and testing instructions, see:
- [docs/history/ICE_SEND_OPTIONS_FIX.md](../docs/history/ICE_SEND_OPTIONS_FIX.md) - ICE layer implementation and backtrace addition
- [docs/history/DTLS_FORWARDING_FIX.md](../docs/history/DTLS_FORWARDING_FIX.md) - DTLS layer forwarding details
- [docs/history/TRACEROUTE_FIX_SUMMARY.md](../docs/history/TRACEROUTE_FIX_SUMMARY.md) - Overall traceroute implementation
- [docs/UDP_PACKET_OPTIONS.md](../docs/UDP_PACKET_OPTIONS.md) - Complete feature documentation

## Platform Support

All modifications are Linux-specific and properly guarded with `#[cfg(target_os = "linux")]`. On other platforms, the code gracefully falls back to standard send operations.
