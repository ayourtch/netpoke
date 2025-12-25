/// Example: Creating UDP sockets with custom options (TTL, DF bit)
///
/// This example demonstrates how to create UDP sockets with low-level options
/// like TTL and the Don't Fragment bit using the socket2 crate.
///
/// Run with:
/// ```
/// cargo run --example udp_socket_options
/// ```

use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

#[derive(Clone, Debug)]
pub struct SocketConfig {
    /// Time To Live (TTL) for outgoing packets
    pub ttl: u32,
    
    /// Enable Don't Fragment (DF) bit in IP header
    pub df_bit: bool,
    
    /// Type of Service / DSCP value (optional)
    pub tos: Option<u8>,
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

/// Creates a UDP socket with the specified configuration options
fn create_configured_socket(
    addr: SocketAddr,
    config: &SocketConfig,
) -> Result<UdpSocket, io::Error> {
    println!("Creating socket with config: {:?}", config);
    println!("  Address: {}", addr);
    
    // Determine the domain (IPv4 or IPv6) based on the address
    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    
    // Create a raw socket using socket2
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
    println!("  ✓ Created raw socket");
    
    // Enable address reuse (useful for development/testing)
    socket.set_reuse_address(true)?;
    println!("  ✓ Enabled SO_REUSEADDR");
    
    // Set TTL (Time To Live)
    socket.set_ttl(config.ttl)?;
    println!("  ✓ Set TTL to {}", config.ttl);
    
    // Set DF (Don't Fragment) bit - platform specific
    #[cfg(target_os = "linux")]
    if config.df_bit {
        set_df_bit_linux(&socket, addr.is_ipv4())?;
        println!("  ✓ Enabled DF (Don't Fragment) bit");
    }
    
    #[cfg(not(target_os = "linux"))]
    if config.df_bit {
        println!("  ⚠ DF bit setting not implemented for this platform");
    }
    
    // Set TOS/DSCP if specified
    #[cfg(target_os = "linux")]
    if let Some(tos) = config.tos {
        set_tos_linux(&socket, tos, addr.is_ipv4())?;
        println!("  ✓ Set TOS/DSCP to {}", tos);
    }
    
    // Bind the socket to the address
    socket.bind(&addr.into())?;
    let bound_addr = socket.local_addr()?;
    println!("  ✓ Bound to {}", bound_addr.as_socket().map(|a| a.to_string()).unwrap_or_else(|| "unknown".to_string()));
    
    // Set non-blocking mode (required for tokio)
    socket.set_nonblocking(true)?;
    println!("  ✓ Set non-blocking mode");
    
    // Convert socket2::Socket -> std::net::UdpSocket -> tokio::net::UdpSocket
    let std_socket: std::net::UdpSocket = socket.into();
    let tokio_socket = UdpSocket::from_std(std_socket)?;
    
    println!("  ✓ Converted to tokio UdpSocket");
    
    Ok(tokio_socket)
}

/// Set the Don't Fragment bit on Linux
#[cfg(target_os = "linux")]
fn set_df_bit_linux(socket: &Socket, is_ipv4: bool) -> Result<(), io::Error> {
    use std::os::unix::io::AsRawFd;
    
    let fd = socket.as_raw_fd();
    
    unsafe {
        if is_ipv4 {
            // For IPv4, use IP_MTU_DISCOVER with IP_PMTUDISC_DO
            // IP_MTU_DISCOVER = 10 (from linux/in.h)
            // IP_PMTUDISC_DO = 2 (always set DF bit)
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
            // For IPv6, use IPV6_MTU_DISCOVER with IPV6_PMTUDISC_DO
            // IPV6_MTU_DISCOVER = 23
            // IPV6_PMTUDISC_DO = 2
            const IPV6_MTU_DISCOVER: libc::c_int = 23;
            const IPV6_PMTUDISC_DO: libc::c_int = 2;
            
            let optval: libc::c_int = IPV6_PMTUDISC_DO;
            let ret = libc::setsockopt(
                fd,
                libc::IPPROTO_IPV6,
                IPV6_MTU_DISCOVER,
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

/// Set the Type of Service (TOS) / DSCP value on Linux
#[cfg(target_os = "linux")]
fn set_tos_linux(socket: &Socket, tos: u8, is_ipv4: bool) -> Result<(), io::Error> {
    use std::os::unix::io::AsRawFd;
    
    let fd = socket.as_raw_fd();
    
    unsafe {
        if is_ipv4 {
            // For IPv4, use IP_TOS
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
            // For IPv6, use IPV6_TCLASS
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

/// Verify socket options by reading them back (Linux only)
#[cfg(target_os = "linux")]
fn verify_socket_options(socket: &UdpSocket, addr: SocketAddr) -> Result<(), io::Error> {
    use std::os::unix::io::AsRawFd;
    
    let fd = socket.as_raw_fd();
    
    println!("\nVerifying socket options:");
    
    unsafe {
        // Verify TTL
        let mut ttl: libc::c_int = 0;
        let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
        
        if addr.is_ipv4() {
            let ret = libc::getsockopt(
                fd,
                libc::IPPROTO_IP,
                libc::IP_TTL,
                &mut ttl as *mut _ as *mut libc::c_void,
                &mut len,
            );
            
            if ret == 0 {
                println!("  Current TTL: {}", ttl);
            }
            
            // Verify DF bit setting
            let mut mtu_disc: libc::c_int = 0;
            let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
            let ret = libc::getsockopt(
                fd,
                libc::IPPROTO_IP,
                libc::IP_MTU_DISCOVER,
                &mut mtu_disc as *mut _ as *mut libc::c_void,
                &mut len,
            );
            
            if ret == 0 {
                let df_status = match mtu_disc {
                    0 => "IP_PMTUDISC_DONT (DF bit never set)",
                    1 => "IP_PMTUDISC_WANT (DF bit set opportunistically)",
                    2 => "IP_PMTUDISC_DO (DF bit always set)",
                    3 => "IP_PMTUDISC_PROBE (DF bit set, ignore PMTU)",
                    _ => "Unknown",
                };
                println!("  MTU Discovery: {} ({})", mtu_disc, df_status);
            }
        } else {
            let ret = libc::getsockopt(
                fd,
                libc::IPPROTO_IPV6,
                libc::IPV6_UNICAST_HOPS,
                &mut ttl as *mut _ as *mut libc::c_void,
                &mut len,
            );
            
            if ret == 0 {
                println!("  Current Hop Limit (TTL): {}", ttl);
            }
        }
    }
    
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn verify_socket_options(_socket: &UdpSocket, _addr: SocketAddr) -> Result<(), io::Error> {
    println!("\nSocket option verification not implemented for this platform");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== UDP Socket Options Example ===\n");
    
    // Example 1: IPv4 socket with custom TTL
    println!("Example 1: IPv4 socket with TTL=32");
    println!("─────────────────────────────────");
    let config1 = SocketConfig {
        ttl: 32,
        df_bit: false,
        tos: None,
    };
    let socket1 = create_configured_socket("0.0.0.0:0".parse()?, &config1)?;
    let addr1 = socket1.local_addr()?;
    verify_socket_options(&socket1, addr1)?;
    
    println!();
    
    // Example 2: IPv4 socket with DF bit enabled
    println!("Example 2: IPv4 socket with DF bit enabled");
    println!("─────────────────────────────────────────");
    let config2 = SocketConfig {
        ttl: 64,
        df_bit: true,
        tos: None,
    };
    let socket2 = create_configured_socket("0.0.0.0:0".parse()?, &config2)?;
    let addr2 = socket2.local_addr()?;
    verify_socket_options(&socket2, addr2)?;
    
    println!();
    
    // Example 3: IPv4 socket with TTL, DF bit, and TOS
    println!("Example 3: IPv4 socket with TTL=48, DF bit, and TOS=0x10 (low delay)");
    println!("────────────────────────────────────────────────────────────────────");
    let config3 = SocketConfig {
        ttl: 48,
        df_bit: true,
        tos: Some(0x10), // IPTOS_LOWDELAY
    };
    let socket3 = create_configured_socket("0.0.0.0:0".parse()?, &config3)?;
    let addr3 = socket3.local_addr()?;
    verify_socket_options(&socket3, addr3)?;
    
    println!();
    
    // Example 4: IPv6 socket
    println!("Example 4: IPv6 socket with TTL=128");
    println!("───────────────────────────────────");
    let config4 = SocketConfig {
        ttl: 128,
        df_bit: false,
        tos: None,
    };
    match create_configured_socket("[::]:0".parse()?, &config4) {
        Ok(socket4) => {
            let addr4 = socket4.local_addr()?;
            verify_socket_options(&socket4, addr4)?;
        }
        Err(e) => {
            println!("  ⚠ IPv6 not available: {}", e);
        }
    }
    
    println!();
    
    // Demonstrate sending a packet
    println!("Example 5: Sending test packet");
    println!("─────────────────────────────────");
    let test_config = SocketConfig {
        ttl: 64,
        df_bit: true,
        tos: None,
    };
    let test_socket = create_configured_socket("0.0.0.0:0".parse()?, &test_config)?;
    
    // Send a test packet to localhost
    let test_data = b"Hello from configured socket!";
    let bytes_sent = test_socket.send_to(test_data, "127.0.0.1:9999").await?;
    println!("  Sent {} bytes to 127.0.0.1:9999", bytes_sent);
    println!("  Packet sent with TTL=64 and DF bit set");
    
    println!();
    println!("=== Example Complete ===");
    println!();
    println!("To verify packet attributes, use tcpdump:");
    println!("  sudo tcpdump -i lo -v udp port {} -c 1", test_socket.local_addr()?.port());
    println!();
    println!("Look for:");
    println!("  - 'ttl 64' in the output");
    println!("  - 'DF' flag in the IP header");
    
    Ok(())
}
