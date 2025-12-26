# Vendored webrtc-util Modifications

Modifications to webrtc-util v0.12.0 for UDP socket options support.

## Version Information

- **Crate**: webrtc-util v0.12.0
- **Original Repository**: https://github.com/webrtc-rs/webrtc
- **Path in Repo**: util/
- **Commit SHA**: `a1f8f1919235d8452835852e018efd654f2f8366`
- **Crates.io**: https://crates.io/crates/webrtc-util/0.12.0

## Patch Files

This directory contains patch files that document the modifications made to the vendored webrtc-util crate:

1. **01-cargo-toml.patch** - Adds libc dependency for Linux
2. **02-lib-rs.patch** - Exports UdpSendOptions and set_send_options
3. **03-conn-mod-rs.patch** - Re-exports from conn_udp module
4. **04-conn-udp-rs.patch** - Main implementation (~195 lines)
   - Thread-local storage for options
   - `sendmsg()` with control messages
   - IPv4 and IPv6 support

## Applying Patches

To apply patches to a fresh copy of webrtc-util v0.12.0:

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

To update the vendored crate:

```bash
./scripts/refresh-vendored.sh
```

See `docs/UDP_PACKET_OPTIONS.md` for complete details.
