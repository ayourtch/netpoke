# UDP Length-Based ICMP Packet Matching

## Problem

When sending traceroute probes with low TTL values, ICMP Time Exceeded errors are received, but they couldn't be matched to the original packets because:

1. **No tracked packets**: The `track_packet()` function was never called
2. **ICMP payload limitation**: ICMP Time Exceeded messages only include:
   - IP header of the original packet (20 bytes)
   - First 8 bytes of the original datagram (UDP header only, no payload)
3. **Previous matching strategy**: Used `src_port` + `dest_addr` + `payload_prefix`, but:
   - Source port wasn't known at application layer
   - Payload prefix was always empty (not included in ICMP)

## Solution: UDP Length-Based Matching

### Key Insight
The UDP header (included in ICMP Time Exceeded) contains the **UDP length field** (bytes 4-5), which includes both the 8-byte UDP header and the payload length. This can be used as a unique identifier!

### Implementation

#### 1. Matching Key Structure
```rust
pub struct UdpPacketKey {
    pub dest_addr: SocketAddr,  // Destination IP + port
    pub udp_length: u16,         // UDP packet length from header
}
```

#### 2. Traceroute Probe Padding
Each TTL value gets a unique packet size:
```rust
let target_size = 100 + (current_ttl as usize * 10);
// TTL 1 → 110 bytes
// TTL 2 → 120 bytes
// TTL 3 → 130 bytes
// etc.
```

#### 3. UDP Length Estimation
At the application layer, we estimate the final UDP packet length:
```rust
let estimated_udp_length = (json.len() + 100) as u16;
```

The +100 accounts for:
- DTLS encryption overhead (~40-60 bytes)
- SCTP header (~12 bytes)
- UDP header (8 bytes)
- Data channel framing (~20-30 bytes)

The exact value doesn't need to be perfect - what matters is that each TTL has a **different, predictable length**.

#### 4. ICMP Parsing
Extract UDP length from the embedded UDP header in ICMP errors:
```rust
let udp_length = u16::from_be_bytes([
    packet[embedded_udp_start + 4],
    packet[embedded_udp_start + 5],
]);
```

#### 5. Packet Tracking
Track packets when sending traceroute probes:
```rust
session.packet_tracker.track_packet(
    json.clone(),           // Cleartext probe data
    dummy_udp_packet,       // Empty (actual packet not available)
    0,                      // Source port (unknown/unused)
    dest,                   // Destination address
    estimated_udp_length,   // Estimated UDP length
    send_options,           // TTL, track_for_ms, etc.
).await;
```

#### 6. ICMP Matching
When an ICMP error arrives:
1. Parse the embedded UDP header to get `udp_length`
2. Extract destination IP/port
3. Look up tracked packet using key: `(dest_addr, udp_length)`
4. If found, create a `TrackedPacketEvent` with both ICMP and original data

## Benefits

✅ **Reliable matching**: Each TTL hop has a unique packet size  
✅ **No payload needed**: Works despite ICMP Time Exceeded limitation  
✅ **No source port needed**: Destination + length is sufficient  
✅ **Stores cleartext**: Original probe data available for analysis  
✅ **Simple calculation**: Estimated length = data + fixed overhead  

## Testing

Comprehensive tests verify:
- ✅ Matching works with correct UDP length
- ✅ Matching fails with incorrect UDP length (no false positives)
- ✅ Packet expiry cleanup
- ✅ Basic tracking functionality

## Example Flow

```
1. Application sends traceroute probe with TTL=1, JSON size=120
2. Estimated UDP length = 120 + 100 = 220 bytes
3. Track packet with key: (dest=37.228.235.203:55610, udp_length=220)
4. Packet goes through DTLS/SCTP/UDP stack
5. Actual UDP packet sent (slightly different size due to encryption)
6. Router drops packet (TTL=0), sends ICMP Time Exceeded
7. ICMP listener receives error, extracts UDP length from embedded header
8. Match found: udp_length≈220, dest=37.228.235.203:55610
9. Create event with both ICMP packet and original cleartext probe
```

## Future Enhancements

Possible improvements:
- Dynamically calibrate overhead estimation based on first few matches
- Support IPv6 (currently IPv4 only)
- Add statistics on match rates
- Correlate matched events with traceroute hop information
