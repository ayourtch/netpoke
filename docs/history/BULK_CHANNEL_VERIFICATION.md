# Bulk Channel WebRTC Configuration Verification

## Summary

This document verifies that the WebRTC bulk channel is now configured to use unreliable, unordered delivery for realistic throughput measurement.

## Configuration Change

### Before
```rust
// Create bulk channel (reliable, ordered)
let bulk_init = RtcDataChannelInit::new();
let bulk_channel = peer.create_data_channel_with_data_channel_dict("bulk", &bulk_init);
```

### After
```rust
// Create bulk channel (unreliable, unordered) for realistic throughput measurement
let bulk_init = RtcDataChannelInit::new();
bulk_init.set_ordered(false);
bulk_init.set_max_retransmits(0);
let bulk_channel = peer.create_data_channel_with_data_channel_dict("bulk", &bulk_init);
```

## WebRTC Data Channel Settings

| Setting | Value | Effect |
|---------|-------|--------|
| `ordered` | `false` | Packets may arrive out of order |
| `maxRetransmits` | `0` | No retransmission of lost packets |

## Rationale

Using unreliable, unordered delivery for the bulk channel provides several benefits for network measurement:

1. **Realistic Throughput**: Measures actual network capacity without retransmission overhead
2. **Packet Loss Detection**: Lost packets are not retransmitted, allowing accurate packet loss measurement
3. **Lower Latency**: No retransmission delays that could mask network issues
4. **Congestion Detection**: Shows actual network conditions without reliability mechanisms masking problems

## Comparison with Other Channels

| Channel | Ordered | MaxRetransmits | Purpose |
|---------|---------|----------------|---------|
| probe | false | 0 | Unreliable probes for delay/jitter/loss measurement |
| bulk | false | 0 | Unreliable bulk data for realistic throughput measurement |
| control | true | unlimited (default) | Reliable control messages and stats reporting |
| testprobe | false | 0 | Unreliable test probes for traceroute functionality |

## WebRTC Channel Type

With the configuration:
- `ordered: false`
- `maxRetransmits: 0`

The WebRTC channel uses the **PartialReliableRexmitUnordered** channel type, which corresponds to:
- SCTP PPID: Partial Reliability (PR-SCTP)
- Unordered delivery
- Zero retransmissions

This is equivalent to UDP-like behavior over WebRTC, which is ideal for network measurement applications.

## Verification

To verify the configuration is correct:

1. **Build the client**:
   ```bash
   cd client
   wasm-pack build --target web --out-dir ../server/static/public/pkg
   ```

2. **Check the compiled code** (in browser DevTools):
   - The bulk channel should have `ordered: false` and `maxRetransmits: 0` in its configuration
   - Network captures should show no retransmissions for dropped bulk packets

3. **Runtime behavior**:
   - Packet loss on bulk channel should be observable in metrics
   - Throughput should reflect actual network capacity without artificial inflation from retransmissions

## References

- [WebRTC Data Channels API](https://developer.mozilla.org/en-US/docs/Web/API/RTCDataChannel)
- [RTCDataChannelInit](https://developer.mozilla.org/en-US/docs/Web/API/RTCDataChannel/RTCDataChannel#parameters)
- [SCTP Partial Reliability Extension (RFC 3758)](https://tools.ietf.org/html/rfc3758)
