# Vendored webrtc-util Modifications

Modifications to webrtc-util v0.12.0 for UDP socket options support.

## Files Modified

1. `Cargo.toml` - Added libc dependency for Linux
2. `src/conn/conn_udp.rs` - UDP options via sendmsg()
3. `src/conn/mod.rs` - Re-exports
4. `src/lib.rs` - Crate-level exports

## Maintenance

Update: `./scripts/refresh-vendored.sh`

See `docs/VENDORED_WEBRTC_UTIL.md` for details.
