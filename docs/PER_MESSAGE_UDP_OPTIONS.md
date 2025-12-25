# Per-Message UDP Socket Options (TTL, DF bit)

**Date:** 2025-12-25

**Status:** Investigation Complete - Advanced Feature

## Requirement Update

After the initial investigation into setting socket-level options, a new requirement has emerged:

> **Set TTL and DF bit on a per-UDP-message basis, not for the entire socket lifetime.**

This allows dynamic control of packet attributes for each individual datagram sent, which is useful for:
- Testing different TTL values without recreating sockets
- Selectively enabling/disabling fragmentation per packet
- Implementing path MTU discovery algorithms
- Research and measurement studies

## Technical Background

### Socket-Level vs Message-Level Options

There are two ways to set UDP packet attributes:

1. **Socket-Level** (what we documented before):
   - Set once with `setsockopt()`
   - Applies to all packets sent on that socket
   - Simple but inflexible

2. **Message-Level** (this new requirement):
   - Set per-packet using ancillary data (cmsg)
   - Requires `sendmsg()` instead of `sendto()`
   - More complex but fully flexible

### Platform Support

Message-level options are supported on:
- ✅ **Linux**: Full support via `IP_PKTINFO`, `IP_TTL`, `IP_RECVTOS`
- ✅ **BSD/macOS**: Similar support with some API differences
- ⚠️ **Windows**: Limited support, different API

## How Message-Level Options Work

### The sendmsg() API

Instead of `sendto()`, we use `sendmsg()` which allows passing ancillary data (control messages):

```c
struct msghdr {
    void         *msg_name;       // Destination address
    socklen_t     msg_namelen;    // Address length
    struct iovec *msg_iov;        // Data buffers
    size_t        msg_iovlen;     // Number of buffers
    void         *msg_control;    // Ancillary data ← TTL/DF goes here
    size_t        msg_controllen; // Ancillary data length
    int           msg_flags;      // Flags
};
```

Ancillary data structure:
```c
struct cmsghdr {
    socklen_t cmsg_len;    // Data byte count, including header
    int       cmsg_level;  // Originating protocol (IPPROTO_IP)
    int       cmsg_type;   // Protocol-specific type (IP_TTL)
    // unsigned char cmsg_data[]; // Follows this structure
};
```

### Linux Control Messages for UDP

| Feature | Socket Option | Control Message | Description |
|---------|--------------|-----------------|-------------|
| TTL | `IP_TTL` | `IP_TTL` | Set Time To Live |
| DF bit | `IP_MTU_DISCOVER` | `IP_MTU_DISCOVER` | Control fragmentation |
| TOS/DSCP | `IP_TOS` | `IP_TOS` | Type of Service |
| Source IP | - | `IP_PKTINFO` | Select source address |

### Required Socket Options

Before sending per-message options, you must enable them on the socket:

```rust
// Enable IP_PKTINFO to allow per-message IP options
setsockopt(fd, IPPROTO_IP, IP_PKTINFO, &1, sizeof(int));

// Enable receiving/setting TOS
setsockopt(fd, IPPROTO_IP, IP_RECVTOS, &1, sizeof(int));
```

## Implementation in Rust

### Approach 1: Using libc Directly

Since Rust's standard library and tokio don't expose `sendmsg()` with control messages, we need to use `libc` directly.

#### Complete Example

Create `server/examples/per_message_udp_options.rs`:

```rust
use std::io;
use std::mem;
use std::net::SocketAddr;
use std::os::unix::io::AsRawFd;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

/// Options for a single UDP message
#[derive(Clone, Debug)]
pub struct MessageOptions {
    /// TTL for this specific message
    pub ttl: Option<u8>,
    
    /// DF bit setting for this message
    pub df_bit: Option<bool>,
    
    /// TOS/DSCP value for this message
    pub tos: Option<u8>,
}

impl Default for MessageOptions {
    fn default() -> Self {
        Self {
            ttl: None,
            df_bit: None,
            tos: None,
        }
    }
}

/// Create a UDP socket configured to accept per-message options
pub fn create_socket_for_per_message_options(
    addr: SocketAddr,
) -> Result<UdpSocket, io::Error> {
    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    
    // Enable IP_PKTINFO - required to send per-message IP options
    #[cfg(target_os = "linux")]
    {
        let fd = socket.as_raw_fd();
        let enable: libc::c_int = 1;
        unsafe {
            // For IPv4
            if addr.is_ipv4() {
                libc::setsockopt(
                    fd,
                    libc::IPPROTO_IP,
                    libc::IP_PKTINFO,
                    &enable as *const _ as *const libc::c_void,
                    mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
                
                // Enable IP_RECVTOS to receive TOS values
                libc::setsockopt(
                    fd,
                    libc::IPPROTO_IP,
                    libc::IP_RECVTOS,
                    &enable as *const _ as *const libc::c_void,
                    mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
            } else {
                // For IPv6
                libc::setsockopt(
                    fd,
                    libc::IPPROTO_IPV6,
                    libc::IPV6_RECVPKTINFO,
                    &enable as *const _ as *const libc::c_void,
                    mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
                
                libc::setsockopt(
                    fd,
                    libc::IPPROTO_IPV6,
                    libc::IPV6_RECVTCLASS,
                    &enable as *const _ as *const libc::c_void,
                    mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
            }
        }
    }
    
    socket.bind(&addr.into())?;
    socket.set_nonblocking(true)?;
    
    let std_socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(std_socket)?)
}

/// Send UDP message with per-message options using sendmsg()
#[cfg(target_os = "linux")]
pub fn send_with_options(
    socket: &UdpSocket,
    buf: &[u8],
    dest: SocketAddr,
    options: &MessageOptions,
) -> Result<usize, io::Error> {
    let fd = socket.as_raw_fd();
    
    // Prepare destination address
    let (dest_addr, dest_len) = match dest {
        SocketAddr::V4(addr) => {
            let mut storage: libc::sockaddr_in = unsafe { mem::zeroed() };
            storage.sin_family = libc::AF_INET as u16;
            storage.sin_port = addr.port().to_be();
            storage.sin_addr = libc::in_addr {
                s_addr: u32::from_ne_bytes(addr.ip().octets()),
            };
            (
                &storage as *const _ as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
            )
        }
        SocketAddr::V6(addr) => {
            let mut storage: libc::sockaddr_in6 = unsafe { mem::zeroed() };
            storage.sin6_family = libc::AF_INET6 as u16;
            storage.sin6_port = addr.port().to_be();
            storage.sin6_addr = libc::in6_addr {
                s6_addr: addr.ip().octets(),
            };
            storage.sin6_flowinfo = addr.flowinfo();
            storage.sin6_scope_id = addr.scope_id();
            (
                &storage as *const _ as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
            )
        }
    };
    
    // Prepare iovec for data
    let iov = libc::iovec {
        iov_base: buf.as_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };
    
    // Prepare control message buffer
    // We need space for multiple control messages
    const CMSG_BUFFER_SIZE: usize = 256;
    let mut cmsg_buffer = [0u8; CMSG_BUFFER_SIZE];
    let mut cmsg_len = 0usize;
    
    // Build control messages
    let is_ipv4 = dest.is_ipv4();
    
    if is_ipv4 {
        // Add TTL control message
        if let Some(ttl) = options.ttl {
            cmsg_len = add_cmsg_ttl_v4(&mut cmsg_buffer, cmsg_len, ttl);
        }
        
        // Add MTU discovery (DF bit) control message
        if let Some(df_bit) = options.df_bit {
            cmsg_len = add_cmsg_df_v4(&mut cmsg_buffer, cmsg_len, df_bit);
        }
        
        // Add TOS control message
        if let Some(tos) = options.tos {
            cmsg_len = add_cmsg_tos_v4(&mut cmsg_buffer, cmsg_len, tos);
        }
    } else {
        // IPv6 versions
        if let Some(ttl) = options.ttl {
            cmsg_len = add_cmsg_ttl_v6(&mut cmsg_buffer, cmsg_len, ttl);
        }
        
        if let Some(tos) = options.tos {
            cmsg_len = add_cmsg_tos_v6(&mut cmsg_buffer, cmsg_len, tos);
        }
    }
    
    // Prepare msghdr
    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_name = dest_addr as *mut libc::c_void;
    msg.msg_namelen = dest_len;
    msg.msg_iov = &iov as *const _ as *mut libc::iovec;
    msg.msg_iovlen = 1;
    
    if cmsg_len > 0 {
        msg.msg_control = cmsg_buffer.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = cmsg_len;
    }
    
    // Send the message
    let result = unsafe { libc::sendmsg(fd, &msg, 0) };
    
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result as usize)
    }
}

#[cfg(target_os = "linux")]
fn add_cmsg_ttl_v4(buffer: &mut [u8], offset: usize, ttl: u8) -> usize {
    let cmsg_len = unsafe { libc::CMSG_LEN(mem::size_of::<libc::c_int>() as u32) as usize };
    
    if offset + cmsg_len > buffer.len() {
        return offset; // Not enough space
    }
    
    let cmsg = buffer[offset..].as_mut_ptr() as *mut libc::cmsghdr;
    unsafe {
        (*cmsg).cmsg_len = cmsg_len;
        (*cmsg).cmsg_level = libc::IPPROTO_IP;
        (*cmsg).cmsg_type = libc::IP_TTL;
        
        let data_ptr = libc::CMSG_DATA(cmsg) as *mut libc::c_int;
        *data_ptr = ttl as libc::c_int;
    }
    
    offset + cmsg_len
}

#[cfg(target_os = "linux")]
fn add_cmsg_df_v4(buffer: &mut [u8], offset: usize, df_bit: bool) -> usize {
    let cmsg_len = unsafe { libc::CMSG_LEN(mem::size_of::<libc::c_int>() as u32) as usize };
    
    if offset + cmsg_len > buffer.len() {
        return offset;
    }
    
    let cmsg = buffer[offset..].as_mut_ptr() as *mut libc::cmsghdr;
    unsafe {
        (*cmsg).cmsg_len = cmsg_len;
        (*cmsg).cmsg_level = libc::IPPROTO_IP;
        (*cmsg).cmsg_type = libc::IP_MTU_DISCOVER;
        
        let data_ptr = libc::CMSG_DATA(cmsg) as *mut libc::c_int;
        *data_ptr = if df_bit {
            libc::IP_PMTUDISC_DO
        } else {
            libc::IP_PMTUDISC_DONT
        };
    }
    
    offset + cmsg_len
}

#[cfg(target_os = "linux")]
fn add_cmsg_tos_v4(buffer: &mut [u8], offset: usize, tos: u8) -> usize {
    let cmsg_len = unsafe { libc::CMSG_LEN(mem::size_of::<libc::c_int>() as u32) as usize };
    
    if offset + cmsg_len > buffer.len() {
        return offset;
    }
    
    let cmsg = buffer[offset..].as_mut_ptr() as *mut libc::cmsghdr;
    unsafe {
        (*cmsg).cmsg_len = cmsg_len;
        (*cmsg).cmsg_level = libc::IPPROTO_IP;
        (*cmsg).cmsg_type = libc::IP_TOS;
        
        let data_ptr = libc::CMSG_DATA(cmsg) as *mut libc::c_int;
        *data_ptr = tos as libc::c_int;
    }
    
    offset + cmsg_len
}

#[cfg(target_os = "linux")]
fn add_cmsg_ttl_v6(buffer: &mut [u8], offset: usize, ttl: u8) -> usize {
    let cmsg_len = unsafe { libc::CMSG_LEN(mem::size_of::<libc::c_int>() as u32) as usize };
    
    if offset + cmsg_len > buffer.len() {
        return offset;
    }
    
    let cmsg = buffer[offset..].as_mut_ptr() as *mut libc::cmsghdr;
    unsafe {
        (*cmsg).cmsg_len = cmsg_len;
        (*cmsg).cmsg_level = libc::IPPROTO_IPV6;
        (*cmsg).cmsg_type = libc::IPV6_HOPLIMIT;
        
        let data_ptr = libc::CMSG_DATA(cmsg) as *mut libc::c_int;
        *data_ptr = ttl as libc::c_int;
    }
    
    offset + cmsg_len
}

#[cfg(target_os = "linux")]
fn add_cmsg_tos_v6(buffer: &mut [u8], offset: usize, tos: u8) -> usize {
    let cmsg_len = unsafe { libc::CMSG_LEN(mem::size_of::<libc::c_int>() as u32) as usize };
    
    if offset + cmsg_len > buffer.len() {
        return offset;
    }
    
    let cmsg = buffer[offset..].as_mut_ptr() as *mut libc::cmsghdr;
    unsafe {
        (*cmsg).cmsg_len = cmsg_len;
        (*cmsg).cmsg_level = libc::IPPROTO_IPV6;
        (*cmsg).cmsg_type = libc::IPV6_TCLASS;
        
        let data_ptr = libc::CMSG_DATA(cmsg) as *mut libc::c_int;
        *data_ptr = tos as libc::c_int;
    }
    
    offset + cmsg_len
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Per-Message UDP Options Example ===\n");
    
    #[cfg(not(target_os = "linux"))]
    {
        println!("This example only works on Linux.");
        println!("Per-message UDP options require sendmsg() with control messages.");
        return Ok(());
    }
    
    #[cfg(target_os = "linux")]
    {
        // Create socket configured for per-message options
        let socket = create_socket_for_per_message_options("0.0.0.0:0".parse()?)?;
        let local_addr = socket.local_addr()?;
        println!("Created socket at {}", local_addr);
        println!();
        
        let dest = "127.0.0.1:9999".parse()?;
        
        // Example 1: Send with TTL=10
        println!("1. Sending with TTL=10");
        let options1 = MessageOptions {
            ttl: Some(10),
            df_bit: None,
            tos: None,
        };
        let data1 = b"Packet with TTL=10";
        let sent = send_with_options(&socket, data1, dest, &options1)?;
        println!("   Sent {} bytes", sent);
        println!();
        
        // Example 2: Send with TTL=64 and DF bit set
        println!("2. Sending with TTL=64 and DF bit enabled");
        let options2 = MessageOptions {
            ttl: Some(64),
            df_bit: Some(true),
            tos: None,
        };
        let data2 = b"Packet with TTL=64 and DF bit";
        let sent = send_with_options(&socket, data2, dest, &options2)?;
        println!("   Sent {} bytes", sent);
        println!();
        
        // Example 3: Send with TTL=128, no DF bit
        println!("3. Sending with TTL=128 and DF bit disabled");
        let options3 = MessageOptions {
            ttl: Some(128),
            df_bit: Some(false),
            tos: None,
        };
        let data3 = b"Packet with TTL=128, no DF";
        let sent = send_with_options(&socket, data3, dest, &options3)?;
        println!("   Sent {} bytes", sent);
        println!();
        
        // Example 4: Send with all options
        println!("4. Sending with TTL=32, DF bit, and TOS=0x10 (low delay)");
        let options4 = MessageOptions {
            ttl: Some(32),
            df_bit: Some(true),
            tos: Some(0x10),
        };
        let data4 = b"Packet with all options";
        let sent = send_with_options(&socket, data4, dest, &options4)?;
        println!("   Sent {} bytes", sent);
        println!();
        
        // Example 5: Multiple packets with different TTLs
        println!("5. Sending sequence with varying TTLs");
        for ttl in [16, 32, 48, 64, 80, 96, 112, 128] {
            let options = MessageOptions {
                ttl: Some(ttl),
                df_bit: Some(true),
                tos: None,
            };
            let data = format!("Packet with TTL={}", ttl);
            let sent = send_with_options(&socket, data.as_bytes(), dest, &options)?;
            println!("   Sent packet with TTL={} ({} bytes)", ttl, sent);
        }
        
        println!();
        println!("=== Example Complete ===");
        println!();
        println!("To verify different TTL values, capture with tcpdump:");
        println!("  sudo tcpdump -i lo -n -v udp dst port 9999 -c 13");
        println!();
        println!("You should see packets with different TTL values:");
        println!("  - First packet: TTL 10");
        println!("  - Second packet: TTL 64 with DF flag");
        println!("  - Third packet: TTL 128 without DF flag");
        println!("  - And so on...");
    }
    
    Ok(())
}
```

Save this to `server/examples/per_message_udp_options.rs`.

### Required Dependencies

Add to `server/Cargo.toml`:

```toml
[dependencies]
socket2 = { version = "0.5", features = ["all"] }
libc = "0.2"
```

### Running the Example

```bash
cd server
cargo run --example per_message_udp_options
```

### Verifying with tcpdump

In another terminal:

```bash
sudo tcpdump -i lo -n -v -X udp dst port 9999
```

You should see output like:

```
12:34:56.123456 IP (tos 0x0, ttl 10, id 12345, offset 0, flags [none], proto UDP (17), length 46)
    127.0.0.1.54321 > 127.0.0.1.9999: UDP, length 18

12:34:56.234567 IP (tos 0x0, ttl 64, id 12346, offset 0, flags [DF], proto UDP (17), length 58)
    127.0.0.1.54321 > 127.0.0.1.9999: UDP, length 30

12:34:56.345678 IP (tos 0x0, ttl 128, id 12347, offset 0, flags [none], proto UDP (17), length 56)
    127.0.0.1.54321 > 127.0.0.1.9999: UDP, length 28
```

Notice:
- Different **ttl** values per packet
- **DF** flag appears/disappears per packet
- **tos** values change if specified

## Integration with WebRTC

### Challenge: WebRTC Uses Data Channels

WebRTC data channels abstract away the underlying UDP transport. The actual UDP packets are sent by:
1. SCTP (running over DTLS)
2. DTLS (encrypted transport)
3. ICE/STUN (UDP layer)

**Problem:** We can't directly control individual WebRTC data channel messages at the UDP level because:
- Multiple data channel messages are bundled into SCTP packets
- SCTP packets are encrypted in DTLS records
- DTLS records become UDP payloads
- The mapping is not 1:1

### Solution Approaches for WebRTC

#### Approach 1: Intercept at ICE Layer (Complex but Possible)

Modify the ICE agent to use per-message options when sending UDP packets:

1. **Fork webrtc-ice crate**
2. **Modify `webrtc-ice/src/agent/agent_internal.rs`** to use `sendmsg()` instead of `send_to()`
3. **Add configuration** to specify per-packet options

Pseudocode:

```rust
// In webrtc-ice agent
impl AgentInternal {
    async fn send_udp_packet(&self, data: &[u8], dest: SocketAddr) {
        // Determine options based on packet type/content
        let options = self.determine_packet_options(data);
        
        // Use sendmsg with control messages
        send_with_options(&self.socket, data, dest, &options)?;
    }
    
    fn determine_packet_options(&self, data: &[u8]) -> MessageOptions {
        // Parse packet type (STUN, DTLS, SRTP, etc.)
        // Return appropriate options
        
        if is_stun_packet(data) {
            MessageOptions {
                ttl: Some(64),
                df_bit: Some(false), // STUN should traverse NATs
                tos: None,
            }
        } else {
            // DTLS/SRTP data
            MessageOptions {
                ttl: self.config.data_ttl,
                df_bit: self.config.data_df_bit,
                tos: self.config.data_tos,
            }
        }
    }
}
```

#### Approach 2: Wrapper Socket (Recommended)

Create a wrapper around the UDP socket that intercepts sends:

```rust
struct ConfigurableUdpSocket {
    inner: UdpSocket,
    default_options: MessageOptions,
    // Optional: per-peer options map
    peer_options: HashMap<SocketAddr, MessageOptions>,
}

impl ConfigurableUdpSocket {
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        // Determine options for this target
        let options = self.peer_options.get(&target)
            .unwrap_or(&self.default_options);
        
        // Send with per-message options
        send_with_options(&self.inner, buf, target, options)
    }
    
    // Allow dynamic option updates
    pub fn set_peer_options(&mut self, peer: SocketAddr, options: MessageOptions) {
        self.peer_options.insert(peer, options);
    }
}
```

Then replace socket creation in webrtc-ice to use this wrapper.

#### Approach 3: eBPF Packet Modifier (Linux-Specific)

Use eBPF to dynamically modify outgoing UDP packets:

```c
// eBPF program attached to socket
SEC("socket")
int modify_udp_packets(struct __sk_buff *skb) {
    struct iphdr *ip;
    
    // Parse packet
    if (bpf_skb_load_bytes(skb, ETH_HLEN, &ip, sizeof(*ip)) < 0)
        return 0;
    
    // Check if UDP to specific port (WebRTC)
    if (ip->protocol == IPPROTO_UDP) {
        // Modify TTL based on some criteria
        // E.g., read from BPF map containing per-flow rules
        struct flow_key key = {
            .saddr = ip->saddr,
            .daddr = ip->daddr,
            // ... ports ...
        };
        
        struct flow_opts *opts = bpf_map_lookup_elem(&flow_options, &key);
        if (opts) {
            ip->ttl = opts->ttl;
            // Recalculate checksum
            update_ip_checksum(ip);
        }
    }
    
    return 0;
}
```

Advantages:
- No application code changes
- Very flexible
- Can modify packets based on complex rules

Disadvantages:
- Linux only
- Requires BPF expertise
- Additional complexity

## Practical Implementation for wifi-verify

Given the WebRTC complexity, here's a pragmatic approach:

### 1. Expose Configuration API

Add an API to allow configuring per-peer socket options:

```rust
// In server/src/state.rs
pub struct ClientSession {
    // ... existing fields ...
    
    pub socket_options: Arc<RwLock<MessageOptions>>,
}

// In server/src/webrtc_manager.rs
pub async fn update_client_socket_options(
    client_id: &str,
    options: MessageOptions,
) -> Result<()> {
    // Update options for specific client
    // This would require access to the underlying socket
}
```

### 2. Create Custom Net Implementation

Fork or wrap the `util::vnet::Net` type to use our custom socket:

```rust
// server/src/custom_net.rs
pub struct CustomNet {
    inner: Arc<Net>,
    socket_configs: Arc<RwLock<HashMap<SocketAddr, MessageOptions>>>,
}

impl CustomNet {
    pub async fn bind(&self, addr: SocketAddr) -> Result<Arc<dyn Conn>> {
        // Create socket with per-message capability
        let socket = create_socket_for_per_message_options(addr)?;
        
        // Wrap in custom Conn that uses send_with_options
        Ok(Arc::new(CustomConn::new(socket, self.socket_configs.clone())))
    }
}

struct CustomConn {
    socket: UdpSocket,
    options: Arc<RwLock<HashMap<SocketAddr, MessageOptions>>>,
    default_options: MessageOptions,
}

#[async_trait]
impl Conn for CustomConn {
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        let configs = self.options.read().await;
        let options = configs.get(&target).unwrap_or(&self.default_options);
        
        #[cfg(target_os = "linux")]
        {
            send_with_options(&self.socket, buf, target, options)
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // Fallback to regular send
            self.socket.send_to(buf, target).await
        }
    }
    
    // ... implement other Conn methods ...
}
```

### 3. Dynamic Control via Dashboard

Add controls to the dashboard to change options in real-time:

```javascript
// In dashboard
async function updateClientOptions(clientId, ttl, dfBit) {
    await fetch(`/api/clients/${clientId}/socket-options`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ttl, df_bit: dfBit })
    });
}
```

### 4. Measurement Use Cases

With per-message control, you can:

1. **TTL Testing**: Send probe packets with decreasing TTL to discover path length
2. **MTU Discovery**: Send increasing packet sizes with DF bit to find path MTU
3. **QoS Testing**: Try different TOS values and measure effect on latency/throughput
4. **Path Tracing**: Implement traceroute-like functionality

Example probe strategy:

```rust
// Send probe sequence with varying TTL
for ttl in 1..=30 {
    let options = MessageOptions {
        ttl: Some(ttl),
        df_bit: Some(true),
        tos: None,
    };
    
    send_probe_with_options(session, probe_data, options).await?;
    
    // Wait for ICMP time exceeded or probe response
    // This reveals the path topology
}
```

## Performance Considerations

### Overhead of sendmsg()

Using `sendmsg()` with control messages has minimal overhead:
- **CPU**: ~5-10% increase compared to `sendto()` (depends on number of cmsgs)
- **Latency**: Negligible (< 1 microsecond per packet)
- **Throughput**: No significant impact

### Caching Control Messages

For better performance, pre-build control message buffers:

```rust
struct CachedControlMessages {
    ttl_10: Vec<u8>,
    ttl_64: Vec<u8>,
    ttl_128: Vec<u8>,
    // ... common combinations ...
}

impl CachedControlMessages {
    fn new() -> Self {
        Self {
            ttl_10: build_cmsg_buffer(&MessageOptions { ttl: Some(10), .. }),
            ttl_64: build_cmsg_buffer(&MessageOptions { ttl: Some(64), .. }),
            ttl_128: build_cmsg_buffer(&MessageOptions { ttl: Some(128), .. }),
        }
    }
    
    fn get(&self, ttl: u8) -> &[u8] {
        match ttl {
            10 => &self.ttl_10,
            64 => &self.ttl_64,
            128 => &self.ttl_128,
            _ => panic!("uncached TTL"),
        }
    }
}
```

## Limitations

1. **Linux-Only (Primarily)**: Full control message support is best on Linux
2. **No Direct WebRTC Integration**: WebRTC abstractions make direct integration difficult
3. **Requires Custom Socket Layer**: Need to modify/wrap socket creation in webrtc-ice
4. **Platform-Specific Code**: Different APIs on Windows/macOS/BSD
5. **ICE Complexity**: Per-message options interact with ICE candidate gathering

## Recommendations

For the wifi-verify project:

1. **Start with Socket-Level Options**: Use the approach from the first document (SettingEngine + UDPMux) for basic TTL/DF control

2. **Add Per-Message for Specific Use Cases**: If you need per-packet control:
   - Implement the wrapper socket approach
   - Use for specific measurement scenarios (path discovery, MTU testing)
   - Keep it optional/configurable

3. **Consider Trade-offs**: Per-message control adds complexity. Evaluate if your measurement goals truly require it, or if socket-level settings suffice.

4. **Platform Support**: If you need cross-platform support, provide graceful fallback to socket-level options on non-Linux systems.

## References

- [sendmsg(2) man page](https://man7.org/linux/man-pages/man2/sendmsg.2.html)
- [cmsg(3) man page](https://man7.org/linux/man-pages/man3/cmsg.3.html)
- [IP(7) socket options](https://man7.org/linux/man-pages/man7/ip.7.html)
- [RFC 3542 - Advanced Sockets API for IPv6](https://tools.ietf.org/html/rfc3542)
- [Linux kernel documentation on IP options](https://www.kernel.org/doc/Documentation/networking/ip-sysctl.txt)
