# ICE Send Options Fix

## Problem

UDP socket options (TTL, TOS, DF bit) were being lost when packets were sent through the WebRTC stack. The error logs showed:

```
[ERROR] webrtc_util::conn: ❌ send_with_options called on connection without forwarding implementation! 
Options will be LOST: TTL=Some(10), TOS=None, DF=Some(true), ...
This indicates a missing send_with_options implementation in the call chain.
```

The call chain was:
```
RTCDataChannel → SCTP → DTLS → Mux Endpoint → ICE AgentConn → ERROR
```

The issue was that the ICE library's `AgentConn` didn't implement `send_with_options`, causing it to fall back to the default implementation which logged the error and dropped the UDP options.

## Root Cause

The `webrtc-ice` crate (version 0.14.0) from crates.io did not have support for per-packet UDP socket options. The `AgentConn` type implemented the `Conn` trait but only had the standard `send()` method, not `send_with_options()`.

When options were passed through the chain:
1. ✅ `RTCDataChannel::send_with_options()` worked
2. ✅ SCTP layer propagated options
3. ✅ DTLS layer forwarded options
4. ✅ Mux Endpoint forwarded options
5. ❌ ICE AgentConn fell back to default `send()` - **OPTIONS LOST**

## Solution

### 1. Vendored webrtc-ice Crate

Since the ICE library is from crates.io and cannot be directly modified, we vendored it:

```bash
cp -r ~/.cargo/registry/src/.../webrtc-ice-0.14.0 vendored/webrtc-ice
```

Updated `vendored/webrtc/Cargo.toml` to use the vendored version:

```toml
[dependencies]
ice = { version = "0.14.0", path = "../webrtc-ice", package = "webrtc-ice" }
```

### 2. Implemented send_with_options in ICE Stack

Added support for UDP socket options at three levels in the ICE stack:

#### Level 1: AgentConn (agent/agent_transport.rs)

Added `send_with_options()` method to `AgentConn`:

```rust
#[cfg(target_os = "linux")]
async fn send_with_options(
    &self,
    buf: &[u8],
    options: &util::UdpSendOptions,
) -> std::result::Result<usize, util::Error> {
    // Check if connection is closed
    if self.done.load(Ordering::SeqCst) {
        return Err(io::Error::other("Conn is closed").into());
    }

    // Don't allow STUN messages with options
    if is_message(buf) {
        return Err(util::Error::Other("ErrIceWriteStunMessage".into()));
    }

    // Send through selected or best available candidate pair
    let result = if let Some(pair) = self.get_selected_pair() {
        pair.write_with_options(buf, options).await
    } else if let Some(pair) = self.get_best_available_candidate_pair().await {
        pair.write_with_options(buf, options).await
    } else {
        Ok(0)
    };

    match result {
        Ok(n) => {
            self.bytes_sent.fetch_add(buf.len(), Ordering::SeqCst);
            Ok(n)
        }
        Err(err) => Err(io::Error::other(err.to_string()).into()),
    }
}
```

#### Level 2: CandidatePair (candidate/mod.rs)

Added `write_with_options()` method to `CandidatePair`:

```rust
#[cfg(target_os = "linux")]
pub async fn write_with_options(&self, b: &[u8], options: &util::UdpSendOptions) -> Result<usize> {
    self.local.write_to_with_options(b, &*self.remote, options).await
}
```

#### Level 3: Candidate Trait (candidate/mod.rs)

Added `write_to_with_options()` method to the `Candidate` trait:

```rust
#[cfg(target_os = "linux")]
async fn write_to_with_options(
    &self,
    raw: &[u8],
    dst: &(dyn Candidate + Send + Sync),
    _options: &util::UdpSendOptions,
) -> Result<usize> {
    // Default implementation falls back to write_to without options
    log::warn!("⚠️  Candidate::write_to_with_options not implemented, falling back to write_to (options will be LOST)");
    self.write_to(raw, dst).await
}
```

#### Level 4: CandidateBase Implementation (candidate/candidate_base.rs)

Implemented `write_to_with_options()` in `CandidateBase`:

```rust
#[cfg(target_os = "linux")]
async fn write_to_with_options(
    &self,
    raw: &[u8],
    dst: &(dyn Candidate + Send + Sync),
    options: &util::UdpSendOptions,
) -> Result<usize> {
    let n = if let Some(conn) = &self.conn {
        let addr = dst.addr();
        conn.send_to_with_options(raw, addr, options).await?
    } else {
        0
    };
    self.seen(true);
    Ok(n)
}
```

### 3. Added Backtrace to Error Logging

Enhanced the default `send_with_options` implementation in `vendored/webrtc-util/src/conn/mod.rs` to capture and log a backtrace:

```rust
#[cfg(target_os = "linux")]
async fn send_with_options(
    &self,
    buf: &[u8],
    _options: &UdpSendOptions,
) -> Result<usize> {
    let backtrace = std::backtrace::Backtrace::capture();
    log::error!(
        "❌ send_with_options called on connection without forwarding implementation! \
         Options will be LOST: TTL={:?}, TOS={:?}, DF={:?}, buf_len={}, local_addr={:?}, remote_addr={:?}. \
         This indicates a missing send_with_options implementation in the call chain.\n\
         Backtrace:\n{:?}",
        _options.ttl, _options.tos, _options.df_bit, buf.len(), 
        self.local_addr().ok(), self.remote_addr(),
        backtrace
    );
    self.send(buf).await
}
```

This makes it much easier to identify where in the call chain UDP options are being lost in the future.

## Complete Call Chain (After Fix)

```
RTCDataChannel::send_with_options(data, options)
  → DataChannel::write_data_channel_with_options(data, is_string, options)
  → Stream::write_sctp_with_options(data, ppi, options)
  → Chunks with options attached
  → Association::bundle_data_chunks_into_packets()
  → Packets with options attached
  → Association write loop
  → DTLSConn::send_with_options()
  → Endpoint::send_with_options()
  → AgentConn::send_with_options()          ✅ NEW: ICE layer
  → CandidatePair::write_with_options()     ✅ NEW: Candidate pair
  → CandidateBase::write_to_with_options()  ✅ NEW: Candidate base
  → UdpSocket::send_to_with_options()       ✅ Reaches actual UDP socket
  → sendmsg() with control messages         ✅ TTL/TOS/DF applied at syscall level
```

## Testing

The fix can be verified by:

1. **Build the project:**
   ```bash
   cargo build --release
   ```

2. **Run the server and check logs:**
   ```bash
   RUST_LOG=debug ./target/release/netpoke-server
   ```

3. **Look for successful UDP options propagation:**
   ```
   [DEBUG] AgentConn::send_with_options: Sending 276 bytes with TTL=Some(10), TOS=None, DF=Some(true)
   [DEBUG] CandidateBase::write_to_with_options: Sending 276 bytes with TTL=Some(10), TOS=None, DF=Some(true) to 37.228.235.203:55615
   [INFO] UdpSocket::send_to_with_options called with TTL=Some(10), TOS=None, DF=Some(true), target=37.228.235.203:55615
   [INFO] ✅ sendmsg SUCCEEDED: sent 276 bytes
   ```

4. **Verify no error messages:**
   - Should NOT see: `❌ send_with_options called on connection without forwarding implementation!`
   - Should see successful packet sending with TTL applied

## Platform Support

All changes are Linux-specific and properly guarded with `#[cfg(target_os = "linux")]`. On other platforms:
- The code compiles without the Linux-specific methods
- Default `send()` behavior is used
- No UDP socket options are applied (as expected on non-Linux platforms)

## Maintenance Notes

### When Updating webrtc-ice

If updating to a newer version of webrtc-ice:

1. Copy new version to `vendored/webrtc-ice/`
2. Re-apply the changes:
   - Add `send_with_options()` to `AgentConn` in `agent/agent_transport.rs`
   - Add `write_with_options()` to `CandidatePair` in `candidate/mod.rs`
   - Add `write_to_with_options()` to `Candidate` trait in `candidate/mod.rs`
   - Implement `write_to_with_options()` in `CandidateBase` in `candidate/candidate_base.rs`
3. Update `vendored/webrtc-ice/Cargo.toml` to point to vendored webrtc-util
4. Test thoroughly

### Creating Patches

To generate patch files for the ICE changes:

```bash
cd vendored/webrtc-ice
git init
git add .
git commit -m "Original webrtc-ice 0.14.0"

# Make changes...

git diff > ../../patches/webrtc-ice/send-options.patch
```

## Related Documentation

- [TRACEROUTE_FIX_SUMMARY.md](TRACEROUTE_FIX_SUMMARY.md) - Overall traceroute implementation
- [DTLS_FORWARDING_FIX.md](DTLS_FORWARDING_FIX.md) - DTLS layer forwarding
- [patches/README.md](../../patches/README.md) - All vendored crate modifications

## Benefits

1. **UDP options now work end-to-end** - TTL, TOS, and DF bit are properly applied at the UDP syscall level
2. **Traceroute works over WebRTC** - Per-packet TTL enables traceroute functionality
3. **Better debugging** - Backtrace in error logs makes it easy to spot future issues
4. **Complete control** - All vendored crates can be modified as needed
5. **Future-proof** - Changes are well-documented and easy to maintain

## Version Information

- **webrtc-ice version**: 0.14.0
- **Original repository**: https://github.com/webrtc-rs/ice
- **Crates.io**: https://crates.io/crates/webrtc-ice/0.14.0
- **Vendored path**: `vendored/webrtc-ice/`
- **Modified files**:
  - `src/agent/agent_transport.rs` - Added `send_with_options()` to `AgentConn`
  - `src/candidate/mod.rs` - Added `write_with_options()` to `CandidatePair` and `write_to_with_options()` to `Candidate` trait
  - `src/candidate/candidate_base.rs` - Implemented `write_to_with_options()` in `CandidateBase`
  - `Cargo.toml` - Updated to use vendored `webrtc-util`
