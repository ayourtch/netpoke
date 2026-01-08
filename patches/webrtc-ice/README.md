# webrtc-ice Patches

Patches to add `send_with_options()` support to webrtc-ice v0.14.0 for per-packet UDP socket options.

## Patches

1. **01-cargo-toml.patch** - Updates Cargo.toml to use vendored webrtc-util
2. **02-agent-transport.patch** - Adds `send_with_options()` to `AgentConn`
3. **03-candidate-mod.patch** - Adds `write_to_with_options()` to `Candidate` trait and `write_with_options()` to `CandidatePair`
4. **04-candidate-base.patch** - Implements `write_to_with_options()` in `CandidateBase`

## Applying Patches

To apply these patches to a fresh copy of webrtc-ice v0.14.0:

```bash
# Download fresh crate
cargo download webrtc-ice@0.14.0

# Apply patches in order
cd webrtc-ice-0.14.0
for patch in ../patches/webrtc-ice/*.patch; do
    patch -p1 < "$patch"
done
```

## Changes Summary

### AgentConn (agent/agent_transport.rs)

Adds `send_with_options()` method that:
- Checks if connection is closed
- Prevents STUN messages from using options
- Forwards to selected or best available candidate pair
- Tracks bytes sent
- Logs success/failure

### CandidatePair (candidate/mod.rs)

Adds `write_with_options()` method that forwards to the local candidate's `write_to_with_options()`.

### Candidate Trait (candidate/mod.rs)

Adds `write_to_with_options()` trait method with:
- Full documentation explaining purpose and parameters
- Default implementation that falls back to `write_to()` with a warning
- Linux-only via `#[cfg(target_os = "linux")]`

### CandidateBase (candidate/candidate_base.rs)

Implements `write_to_with_options()` that:
- Logs the send operation with debug level
- Forwards to underlying connection's `send_to_with_options()`
- Updates last seen timestamp
- Returns number of bytes written

## Testing

After applying patches:

1. Verify compilation:
   ```bash
   cargo check --package webrtc-ice
   ```

2. Run tests:
   ```bash
   cargo test --package webrtc-ice
   ```

3. Verify in integration with netpoke server

## Related Documentation

- [../../docs/history/ICE_SEND_OPTIONS_FIX.md](../../docs/history/ICE_SEND_OPTIONS_FIX.md) - Complete ICE implementation details
- [../README.md](../README.md) - All vendored crate modifications
