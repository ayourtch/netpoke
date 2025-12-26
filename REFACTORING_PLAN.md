# UDP Socket Options Refactoring - Detailed Implementation Plan

## Problem Summary
The current implementation uses thread-local storage (`SEND_OPTIONS`) to pass UDP socket options (TTL, TOS, DF bit) to the underlying UDP socket. This approach has several issues:
1. Thread-local storage affects ALL packets sent through that thread, not just specific packets
2. Async tasks can migrate between threads, making thread-local unreliable
3. Cannot apply different options to different packets being sent concurrently

## Solution: Per-Packet Options Passing

### Completed Work

1. **Vendored Required Crates** ✅
   - `vendored/webrtc-data` v0.12.0
   - `vendored/webrtc-sctp` v0.13.0
   - Added patches in `Cargo.toml` to use vendored versions

2. **Extended Conn Trait** ✅
   - Added `send_to_with_options()` method to Conn trait in `vendored/webrtc-util/src/conn/mod.rs`
   - Implemented method for UdpSocket in `vendored/webrtc-util/src/conn/conn_udp.rs`
   - Kept backward compatibility with thread-local storage

3. **Modified webrtc-sctp** ✅
   - Added `udp_send_options: Option<UdpSendOptions>` field to `ChunkPayloadData` struct
   - Added `udp_send_options: Option<UdpSendOptions>` field to `Packet` struct
   - Added `write_sctp_with_options()` method to Stream that accepts options
   - Modified `packetize()` to attach options to all chunks of a message
   - Updated all Packet constructors to include the new field

### Remaining Work

4. **Modify Association Write Loop** 🚧
   Location: `vendored/webrtc-sctp/src/association/mod.rs` (write_loop function around line 503)
   
   Current code:
   ```rust
   if let Err(err) = net_conn.send(raw.as_ref()).await {
   ```
   
   Needs to:
   - Extract UDP options from the first chunk in the packet (if present)
   - Use `net_conn.send_to_with_options()` if options are present
   - Fall back to regular `net_conn.send()` if no options
   
   Implementation:
   ```rust
   #[cfg(target_os = "linux")]
   let send_result = {
       // Extract options from first data chunk in packet
       let udp_options = packet.udp_send_options;
       if let Some(options) = udp_options {
           // Use send_to_with_options - but we need destination address
           // The Association doesn't have direct access to destination, need to refactor
           net_conn.send(raw.as_ref()).await // TEMPORARY
       } else {
           net_conn.send(raw.as_ref()).await
       }
   };
   ```
   
   **ISSUE**: The Association uses `Conn::send()` which doesn't take a destination address. The connection is already "connected" via `UdpSocket::connect()`. We need to either:
   - Modify Conn trait to add `send_with_options(buf, options)` (no address needed)
   - Store the destination address in Association and use `send_to_with_options()`
   - Use a different approach

5. **Modify webrtc-data DataChannel** 🚧
   Location: `vendored/webrtc-data/src/data_channel/mod.rs`
   
   Current methods:
   - `write()` → `write_data_channel()` → `stream.write_sctp()`
   
   Needs:
   - Add `write_with_options(data: &Bytes, options: Option<UdpSendOptions>)`
   - Call `stream.write_sctp_with_options(data, ppi, options)`
   
   Implementation:
   ```rust
   #[cfg(target_os = "linux")]
   pub async fn write_with_options(
       &self,
       data: &Bytes,
       options: Option<UdpSendOptions>,
   ) -> Result<usize> {
       self.write_data_channel_with_options(data, false, options).await
   }
   
   #[cfg(target_os = "linux")]
   pub async fn write_data_channel_with_options(
       &self,
       data: &Bytes,
       is_string: bool,
       options: Option<UdpSendOptions>,
   ) -> Result<usize> {
       // ... existing logic ...
       let ppi = match (is_string, data_len) { ... };
       
       let n = if data_len == 0 {
           self.stream.write_sctp_with_options(&Bytes::from_static(&[0]), ppi, options).await?;
           0
       } else {
           let n = self.stream.write_sctp_with_options(data, ppi, options).await?;
           self.bytes_sent.fetch_add(n, Ordering::SeqCst);
           n
       };
       
       // ... rest of function ...
   }
   ```

6. **Update Server Code** 🚧
   Location: `server/src/measurements.rs`
   
   Current code (around line 394-409):
   ```rust
   if let Ok(json) = serde_json::to_vec(&probe) {
       #[cfg(target_os = "linux")]
       {
           webrtc_util::set_send_options(Some(webrtc_util::UdpSendOptions {
               ttl: Some(current_ttl),
               tos: None,
               df_bit: Some(true),
           }));
       }
       
       let send_result = probe_channel.send(&json.into()).await;
   }
   ```
   
   Needs to:
   - Remove `set_send_options` call
   - Pass options directly through DataChannel API
   - This requires accessing the underlying DataChannel.write_with_options()
   
   **ISSUE**: The webrtc crate's `RTCDataChannel::send()` doesn't expose the underlying DataChannel directly. Options:
   - Modify webrtc crate (not ideal, it's not vendored)
   - Add a wrapper that allows passing options
   - Use a different approach to inject options into the send path

7. **Alternative Approach: Inject Options at Stream Level** 💡
   
   Instead of modifying all the way up to DataChannel::send(), we could:
   - Keep ProbePacket.send_options in the message (already exists)
   - When DataChannel receives the message and calls write_data_channel(), extract send_options from the message
   - Problem: The message is opaque bytes by the time it reaches write_data_channel()
   
   **Better Alternative**:
   - Add a method to set "next packet options" on the Stream or Association
   - Call this right before sending via DataChannel
   - The Stream picks up these options when creating chunks
   
   Implementation:
   ```rust
   // In Stream struct
   next_packet_options: Arc<Mutex<Option<UdpSendOptions>>>,
   
   // New method
   pub async fn set_next_packet_options(&self, options: Option<UdpSendOptions>) {
       *self.next_packet_options.lock().await = options;
   }
   
   // In prepare_write()
   let options = self.next_packet_options.lock().await.take();
   Ok(self.packetize(p, ppi, options))
   ```
   
   Then in server code:
   ```rust
   // Get access to the underlying Stream somehow...
   stream.set_next_packet_options(Some(UdpSendOptions { ttl: Some(current_ttl), ... })).await;
   probe_channel.send(&json.into()).await;
   ```

8. **Handle Cleartext Packet Propagation** 🚧
   
   The problem statement mentions: "the original cleartext packet should propagate as well together with the encrypted one if the corresponding record field is set"
   
   This likely refers to DTLS encryption. Need to:
   - Investigate where DTLS encryption happens in the stack
   - Ensure UDP options are preserved through encryption
   - Possibly add the cleartext packet to some tracking structure

### Key Architectural Decision Needed

The main blocker is deciding how to pass options from the application layer (server measurements code) down through:
1. WebRTC RTCDataChannel API (not vendored, can't easily modify)
2. webrtc-data DataChannel (vendored, can modify)
3. webrtc-sctp Stream/Association (vendored, modified)
4. webrtc-util Conn trait (vendored, modified)
5. UdpSocket (tokio, can't modify)

**Recommended Approach**:
Use a hybrid approach:
1. Keep thread-local storage for backward compatibility
2. Add "per-stream options" that can be set before a send
3. Priority: per-stream options > thread-local > default

This allows:
- Existing code to continue working (thread-local)
- New code to use per-stream options for precise control
- Gradual migration path

### Testing Plan

1. **Unit Tests**
   - Test Stream.write_sctp_with_options() passes options to chunks
   - Test Association write loop uses options when sending
   - Test thread-local still works (backward compat)

2. **Integration Tests**
   - Send packets with different TTLs concurrently
   - Verify each packet has correct TTL using tcpdump
   - Test fragmentation: all fragments should have same options

3. **System Tests**
   - Run traceroute function
   - Capture packets with tcpdump
   - Verify TTL values are applied correctly
   - Verify no interference between concurrent sends

### Files Modified

- [x] Cargo.toml - Added vendored crate patches
- [x] vendored/webrtc-util/src/conn/mod.rs - Extended Conn trait
- [x] vendored/webrtc-util/src/conn/conn_udp.rs - Implemented send_to_with_options
- [x] vendored/webrtc-sctp/src/chunk/chunk_payload_data.rs - Added udp_send_options field
- [x] vendored/webrtc-sctp/src/packet.rs - Added udp_send_options field
- [x] vendored/webrtc-sctp/src/stream/mod.rs - Added write_sctp_with_options method
- [x] vendored/webrtc-sctp/src/association/association_internal.rs - Updated Packet constructors
- [ ] vendored/webrtc-sctp/src/association/mod.rs - Modify write loop (TODO)
- [ ] vendored/webrtc-data/src/data_channel/mod.rs - Add write_with_options (TODO)
- [ ] server/src/measurements.rs - Update to use new API (TODO)

### Notes

- All Linux-specific code is wrapped in `#[cfg(target_os = "linux")]`
- Backward compatibility maintained via thread-local storage
- UdpSendOptions struct is public and exported from webrtc-util
- Options are cloned through the stack (Copy trait) to avoid lifetime issues
