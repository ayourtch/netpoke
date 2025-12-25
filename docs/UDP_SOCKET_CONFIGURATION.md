# UDP Socket Configuration for WebRTC

**Date:** 2025-12-25

**Status:** Investigation Complete

## Problem Statement

The wifi-verify server uses WebRTC data channels to send measurement traffic (probe packets and bulk data) over UDP. To properly test and optimize network behavior, we need the ability to set low-level UDP socket attributes such as:

- **TTL (Time To Live)**: Controls how many network hops a packet can traverse before being discarded
- **DF bit (Don't Fragment)**: Controls whether IP fragmentation is allowed for packets
- **TOS/DSCP**: Traffic classification for QoS purposes

## Current Architecture

### WebRTC Stack Overview

The wifi-verify server uses the Rust WebRTC implementation:

1. **webrtc v0.14.0** - Main WebRTC API layer
2. **webrtc-ice v0.14.0** - ICE protocol implementation (handles UDP socket creation)
3. **webrtc-util v0.12.0** - Utilities including virtual network abstraction
4. **tokio v1.48.0** - Async runtime with `tokio::net::UdpSocket`

### UDP Socket Creation Flow

```
Application (server/src/webrtc_manager.rs)
  ↓
webrtc::api::APIBuilder::new().build()
  ↓
RTCPeerConnection.new()
  ↓
ICE Agent initialization
  ↓
webrtc-ice: agent::gather()
  ↓
util::listen_udp_in_port_range()
  ↓
vnet::Net::bind(addr)
  ↓
tokio::net::UdpSocket::bind(addr)  ← Socket created here
```

**Critical Point:** The socket is created at the lowest level in `webrtc-util/src/vnet/net.rs` line 523:

```rust
Net::Ifs(_) => Ok(Arc::new(UdpSocket::bind(addr).await?)),
```

### Why This Is Challenging

1. **Socket Options Must Be Set Before Binding**: Options like TTL and DF bit need to be configured on the socket **before** calling `bind()`.

2. **Deep in the Stack**: Socket creation happens deep within the webrtc-ice library, not in application code.

3. **No Configuration Hook**: The current WebRTC Rust API doesn't expose a way to configure socket options during creation.

4. **Trait-Based Abstraction**: Sockets are wrapped in the `Conn` trait, making it hard to access the underlying socket.

## Solution Approaches

### Approach 1: Custom UDPMux Implementation (Recommended)

**Description:** Create a custom UDP multiplexer that creates sockets with desired options.

**Implementation:**

```rust
use socket2::{Socket, Domain, Type, Protocol};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

struct CustomUdpMux {
    socket: Arc<UdpSocket>,
    ttl: u32,
    df_bit: bool,
}

impl CustomUdpMux {
    async fn new(addr: SocketAddr, ttl: u32, df_bit: bool) -> Result<Self> {
        // Create socket with socket2 for low-level control
        let domain = if addr.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        };
        
        let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
        
        // Set socket options BEFORE binding
        socket.set_ttl(ttl)?;
        
        #[cfg(target_os = "linux")]
        {
            // Set IP_MTU_DISCOVER to IP_PMTUDISC_DO for DF bit
            if df_bit {
                use std::os::unix::io::AsRawFd;
                let fd = socket.as_raw_fd();
                unsafe {
                    let optval: libc::c_int = libc::IP_PMTUDISC_DO;
                    libc::setsockopt(
                        fd,
                        libc::IPPROTO_IP,
                        libc::IP_MTU_DISCOVER,
                        &optval as *const _ as *const libc::c_void,
                        std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                    );
                }
            }
        }
        
        socket.bind(&addr.into())?;
        socket.set_nonblocking(true)?;
        
        // Convert to tokio UdpSocket
        let std_socket: std::net::UdpSocket = socket.into();
        let tokio_socket = UdpSocket::from_std(std_socket)?;
        
        Ok(Self {
            socket: Arc::new(tokio_socket),
            ttl,
            df_bit,
        })
    }
}

// Implement webrtc_ice::udp_mux::UDPMux trait for CustomUdpMux
```

**Usage in Application:**

```rust
// In webrtc_manager.rs
use webrtc::api::APIBuilder;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::ice_transport::ice_network::UDPNetwork;

pub async fn create_peer_connection_with_socket_options(
    ttl: u32,
    df_bit: bool,
) -> Result<Arc<RTCPeerConnection>> {
    let mut media_engine = MediaEngine::default();
    let registry = Registry::new();
    let registry = register_default_interceptors(registry, &mut media_engine)?;
    
    // Create custom UDP mux
    let udp_mux = CustomUdpMux::new("0.0.0.0:0".parse()?, ttl, df_bit).await?;
    
    // Configure SettingEngine to use custom mux
    let mut setting_engine = SettingEngine::default();
    setting_engine.set_udp_network(UDPNetwork::Muxed(Arc::new(udp_mux)));
    
    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .with_setting_engine(setting_engine)
        .build();
    
    // Rest of peer connection setup...
}
```

**Advantages:**
- ✅ Clean integration with existing WebRTC API
- ✅ Uses documented SettingEngine interface
- ✅ All sockets share same configuration
- ✅ No modification to upstream libraries

**Disadvantages:**
- ⚠️ All connections share one socket (port multiplexing)
- ⚠️ Requires implementing the full UDPMux trait

### Approach 2: Fork and Modify webrtc-util

**Description:** Fork the `webrtc-util` crate and modify `Net::bind()` to accept socket configuration.

**Changes Required:**

1. Modify `webrtc-util/src/vnet/net.rs`:

```rust
pub struct SocketConfig {
    pub ttl: Option<u32>,
    pub df_bit: bool,
    // Add more options as needed
}

impl Net {
    pub async fn bind_with_config(
        &self,
        addr: SocketAddr,
        config: &SocketConfig,
    ) -> Result<Arc<dyn Conn + Send + Sync>> {
        match self {
            Net::VNet(vnet) => {
                let net = vnet.lock().await;
                net.bind(addr).await
            }
            Net::Ifs(_) => {
                // Create socket with socket2
                let socket = create_configured_socket(addr, config)?;
                Ok(Arc::new(UdpSocket::from_std(socket.into())?))
            }
        }
    }
}

fn create_configured_socket(
    addr: SocketAddr,
    config: &SocketConfig,
) -> Result<Socket> {
    let domain = if addr.is_ipv4() { Domain::IPV4 } else { Domain::IPV6 };
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
    
    if let Some(ttl) = config.ttl {
        socket.set_ttl(ttl)?;
    }
    
    if config.df_bit {
        #[cfg(target_os = "linux")]
        set_df_bit(&socket)?;
    }
    
    socket.bind(&addr.into())?;
    socket.set_nonblocking(true)?;
    
    Ok(socket)
}
```

2. Propagate configuration through:
   - `webrtc-ice/src/util/mod.rs::listen_udp_in_port_range()`
   - `webrtc-ice/src/agent/agent_gather.rs`
   - `webrtc/src/api/setting_engine/mod.rs`

**Advantages:**
- ✅ Complete control over socket creation
- ✅ Can configure each socket independently
- ✅ Most flexible approach

**Disadvantages:**
- ❌ Requires maintaining a fork of multiple crates
- ❌ Need to keep fork in sync with upstream
- ❌ More complex maintenance burden

### Approach 3: System-Wide Socket Options

**Description:** Use system-level configuration to set default socket options.

**Linux Example:**

```bash
# Set default TTL
sudo sysctl -w net.ipv4.ip_default_ttl=64

# Set default MTU discovery (DF bit)
sudo sysctl -w net.ipv4.ip_no_pmtu_disc=0
```

**Advantages:**
- ✅ No code changes required
- ✅ Simple to apply

**Disadvantages:**
- ❌ Affects all applications on the system
- ❌ Not suitable for fine-grained control
- ❌ Requires root access
- ❌ Not portable across operating systems

### Approach 4: eBPF/tc (Traffic Control) Hooks

**Description:** Use Linux kernel features to modify packet headers.

**Implementation:**
- Write eBPF program to modify TTL/DF bit in outgoing packets
- Attach to network interface or cgroup

**Advantages:**
- ✅ No application changes
- ✅ Very flexible packet manipulation

**Disadvantages:**
- ❌ Linux-only
- ❌ Requires BPF expertise
- ❌ Additional operational complexity
- ❌ May require root privileges

## Recommended Implementation: Custom UDPMux

### Step-by-Step Implementation

#### 1. Add Dependencies

In `server/Cargo.toml`:

```toml
[dependencies]
socket2 = { version = "0.5", features = ["all"] }
```

#### 2. Create Custom UDP Mux Module

Create `server/src/custom_udp_mux.rs`:

```rust
use async_trait::async_trait;
use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use webrtc_ice::udp_mux::{UDPMux, UDPMuxConn};
use webrtc_ice::Error;

pub struct CustomUdpMux {
    socket: Arc<UdpSocket>,
    config: SocketConfig,
}

#[derive(Clone, Debug)]
pub struct SocketConfig {
    pub ttl: u32,
    pub df_bit: bool,
    pub tos: Option<u8>,  // Type of Service / DSCP
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self {
            ttl: 64,
            df_bit: false,
            tos: None,
        }
    }
}

impl CustomUdpMux {
    pub async fn new(
        listen_addr: SocketAddr,
        config: SocketConfig,
    ) -> Result<Self, io::Error> {
        let socket = Self::create_configured_socket(listen_addr, &config)?;
        
        Ok(Self { socket, config })
    }
    
    fn create_configured_socket(
        addr: SocketAddr,
        config: &SocketConfig,
    ) -> Result<Arc<UdpSocket>, io::Error> {
        // Create raw socket with socket2
        let domain = if addr.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        };
        
        let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
        
        // Enable reuse address for easier testing
        socket.set_reuse_address(true)?;
        
        // Set TTL
        socket.set_ttl(config.ttl)?;
        
        // Set DF bit (Linux-specific)
        #[cfg(target_os = "linux")]
        if config.df_bit {
            Self::set_df_bit(&socket, addr.is_ipv4())?;
        }
        
        // Set TOS/DSCP if specified
        #[cfg(target_os = "linux")]
        if let Some(tos) = config.tos {
            Self::set_tos(&socket, tos, addr.is_ipv4())?;
        }
        
        // Bind the socket
        socket.bind(&addr.into())?;
        
        // Set non-blocking mode for tokio
        socket.set_nonblocking(true)?;
        
        // Convert to std socket then to tokio socket
        let std_socket: std::net::UdpSocket = socket.into();
        let tokio_socket = UdpSocket::from_std(std_socket)?;
        
        Ok(Arc::new(tokio_socket))
    }
    
    #[cfg(target_os = "linux")]
    fn set_df_bit(socket: &Socket, is_ipv4: bool) -> Result<(), io::Error> {
        use std::os::unix::io::AsRawFd;
        
        let fd = socket.as_raw_fd();
        
        unsafe {
            if is_ipv4 {
                // IP_MTU_DISCOVER = 10, IP_PMTUDISC_DO = 2
                let optval: libc::c_int = libc::IP_PMTUDISC_DO;
                let ret = libc::setsockopt(
                    fd,
                    libc::IPPROTO_IP,
                    libc::IP_MTU_DISCOVER,
                    &optval as *const _ as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
                
                if ret != 0 {
                    return Err(io::Error::last_os_error());
                }
            } else {
                // IPV6_MTU_DISCOVER = 23, IPV6_PMTUDISC_DO = 2
                let optval: libc::c_int = 2; // IPV6_PMTUDISC_DO
                let ret = libc::setsockopt(
                    fd,
                    libc::IPPROTO_IPV6,
                    23, // IPV6_MTU_DISCOVER
                    &optval as *const _ as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
                
                if ret != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    fn set_tos(socket: &Socket, tos: u8, is_ipv4: bool) -> Result<(), io::Error> {
        use std::os::unix::io::AsRawFd;
        
        let fd = socket.as_raw_fd();
        
        unsafe {
            if is_ipv4 {
                let optval: libc::c_int = tos as libc::c_int;
                let ret = libc::setsockopt(
                    fd,
                    libc::IPPROTO_IP,
                    libc::IP_TOS,
                    &optval as *const _ as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
                
                if ret != 0 {
                    return Err(io::Error::last_os_error());
                }
            } else {
                let optval: libc::c_int = tos as libc::c_int;
                let ret = libc::setsockopt(
                    fd,
                    libc::IPPROTO_IPV6,
                    libc::IPV6_TCLASS,
                    &optval as *const _ as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
                
                if ret != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl UDPMux for CustomUdpMux {
    fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr().unwrap()
    }
    
    async fn get_conn(
        &self,
        ufrag: &str,
    ) -> Result<Arc<dyn UDPMuxConn + Send + Sync>, Error> {
        // This is a simplified implementation
        // A full implementation would need to handle multiple connections
        // multiplexed over the same socket
        unimplemented!("get_conn needs full UDPMux implementation")
    }
    
    async fn close(&self) -> Result<(), Error> {
        // Socket will be closed when Arc is dropped
        Ok(())
    }
}
```

#### 3. Integrate into WebRTC Manager

Modify `server/src/webrtc_manager.rs`:

```rust
use crate::custom_udp_mux::{CustomUdpMux, SocketConfig};
use webrtc::api::setting_engine::SettingEngine;
use webrtc::ice_transport::ice_network::UDPNetwork;

pub async fn create_peer_connection_with_config(
    socket_config: SocketConfig,
) -> Result<Arc<RTCPeerConnection>, Box<dyn std::error::Error>> {
    let mut media_engine = MediaEngine::default();
    let registry = Registry::new();
    let registry = register_default_interceptors(registry, &mut media_engine)?;
    
    // Create custom UDP mux with socket configuration
    let listen_addr = "0.0.0.0:0".parse()?;
    let udp_mux = CustomUdpMux::new(listen_addr, socket_config).await?;
    
    // Configure SettingEngine
    let mut setting_engine = SettingEngine::default();
    setting_engine.set_udp_network(UDPNetwork::Muxed(Arc::new(udp_mux)));
    
    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .with_setting_engine(setting_engine)
        .build();
    
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ice_transport_policy: "all".into(),
        ..Default::default()
    };
    
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);
    
    Ok(peer_connection)
}
```

#### 4. Add Configuration Options

In `server/src/config.rs`, add socket configuration:

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct SocketOptions {
    #[serde(default = "default_ttl")]
    pub ttl: u32,
    
    #[serde(default)]
    pub df_bit: bool,
    
    #[serde(default)]
    pub tos: Option<u8>,
}

fn default_ttl() -> u32 {
    64
}

impl Default for SocketOptions {
    fn default() -> Self {
        Self {
            ttl: default_ttl(),
            df_bit: false,
            tos: None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    // ... existing fields ...
    
    #[serde(default)]
    pub socket_options: SocketOptions,
}
```

#### 5. Update Configuration File

In `server_config.toml`:

```toml
[server]
host = "0.0.0.0"
http_port = 3000
https_port = 3443
enable_http = true
enable_https = true

[server.socket_options]
ttl = 64          # Set TTL for UDP packets
df_bit = true     # Enable Don't Fragment bit
tos = 0           # Optional: Set Type of Service / DSCP value
```

## Testing and Verification

### Verifying TTL

```bash
# On Linux, use tcpdump to verify TTL
sudo tcpdump -i any -v udp port <webrtc_port> -c 10
# Look for "ttl 64" in output

# Or use tshark
sudo tshark -i any -f "udp port <webrtc_port>" -T fields -e ip.ttl
```

### Verifying DF Bit

```bash
# Use tcpdump with verbose output
sudo tcpdump -i any -v udp port <webrtc_port>
# Look for "DF" flag in IP header

# Or use tshark
sudo tshark -i any -f "udp port <webrtc_port>" -T fields -e ip.flags.df
# Output: 1 = DF bit set, 0 = not set
```

### Testing Fragmentation with DF Bit

```bash
# Send large packets to trigger fragmentation
# If DF bit is set, packets larger than MTU will be dropped
# You should see ICMP "fragmentation needed" messages
```

## Limitations and Considerations

### 1. UDPMux Mode Limitations

When using a custom UDPMux:
- **Single Port**: All WebRTC connections share the same UDP port
- **Multiplexing Overhead**: Requires implementing proper connection multiplexing
- **ICE Limitations**: May affect ICE candidate gathering (fewer candidates)

### 2. Platform-Specific Code

Socket options like DF bit are platform-specific:
- **Linux**: Uses `IP_MTU_DISCOVER` / `IPV6_MTU_DISCOVER`
- **Windows**: Uses `IP_DONTFRAGMENT`
- **macOS**: Uses `IP_DONTFRAG`

The example code needs platform-specific conditional compilation.

### 3. Complete UDPMux Implementation

The example provides a skeleton. A production implementation needs:
- Connection multiplexing based on ICE ufrag
- Proper packet routing to connections
- Connection cleanup and lifecycle management
- Error handling and logging

### 4. Impact on ICE Negotiation

Using UDPMux changes ICE behavior:
- Fewer local candidates (only one port)
- May affect connectivity through some NATs
- All connections share the same 5-tuple

### 5. IPv6 Considerations

Make sure socket options work correctly for both IPv4 and IPv6:
- Different socket option constants
- Different behavior in some cases
- Test both protocol versions

## Alternative: Simpler Partial Solution

If implementing full UDPMux is too complex, consider a hybrid approach:

1. Use the default socket creation for ICE
2. After connection is established, use `tc` (traffic control) on Linux to modify packets:

```bash
# Example: Set TTL on outgoing packets
sudo tc qdisc add dev eth0 root handle 1: prio
sudo tc filter add dev eth0 parent 1: protocol ip prio 1 \
    u32 match ip dport <webrtc_port> 0xffff \
    action pedit ex munge ip ttl set 32
```

This allows setting packet attributes without modifying application code, but requires:
- Linux-only
- Root access
- External configuration
- Less portable

## Conclusion

Setting UDP socket options for WebRTC requires intercepting socket creation, which happens deep in the webrtc-ice library. The recommended approach is to implement a custom UDPMux that creates properly configured sockets. While this requires more implementation work, it provides:

- Clean integration with WebRTC API
- Configuration flexibility
- No need to fork upstream libraries
- Portable across platforms (with platform-specific socket option code)

The main trade-offs are:
- Need to implement UDPMux trait fully
- All connections share one socket/port
- Additional complexity in connection multiplexing

For production use, carefully consider whether the benefits of setting these socket options outweigh the implementation complexity.

## References

- [socket2 crate documentation](https://docs.rs/socket2/)
- [webrtc-rs documentation](https://docs.rs/webrtc/)
- [Linux socket options](https://man7.org/linux/man-pages/man7/ip.7.html)
- [RFC 791 - Internet Protocol](https://tools.ietf.org/html/rfc791)
- [Path MTU Discovery (RFC 1191)](https://tools.ietf.org/html/rfc1191)
