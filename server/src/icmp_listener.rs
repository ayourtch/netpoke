/// ICMP error listener for packet tracking correlation
/// 
/// This module listens for ICMP errors using a raw socket and correlates them
/// with tracked UDP packets. Requires CAP_NET_RAW or root privileges.

use std::sync::Arc;
use crate::packet_tracker::{PacketTracker, EmbeddedUdpInfo};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

/// Start the ICMP listener in the background
pub fn start_icmp_listener(packet_tracker: Arc<PacketTracker>) {
    #[cfg(target_os = "linux")]
    {
        tokio::spawn(async move {
            if let Err(e) = icmp_listener_task(packet_tracker).await {
                tracing::error!("ICMP listener error: {}", e);
            }
        });
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        tracing::warn!("ICMP listener not implemented for this platform");
        let _ = packet_tracker; // Suppress unused warning
    }
}

#[cfg(target_os = "linux")]
async fn icmp_listener_task(packet_tracker: Arc<PacketTracker>) -> std::io::Result<()> {
    use socket2::{Socket, Domain, Type, Protocol};
    use std::os::unix::io::AsRawFd;
    
    tracing::info!("Starting ICMP listener...");
    println!("DEBUG: ICMP listener starting");
    
    // Create raw ICMP socket
    let socket = match Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)) {
        Ok(s) => {
            println!("DEBUG: ICMP socket created successfully");
            s
        },
        Err(e) => {
            tracing::error!(
                "Failed to create ICMP socket (requires CAP_NET_RAW or root): {}",
                e
            );
            println!("DEBUG: Failed to create ICMP socket: {}", e);
            return Err(e);
        }
    };
    
    socket.set_nonblocking(true)?;
    
    let std_socket: std::net::UdpSocket = socket.into();
    let tokio_socket = tokio::net::UdpSocket::from_std(std_socket)?;
    
    tracing::info!("ICMP listener started successfully");
    println!("DEBUG: ICMP listener ready to receive packets");
    
    let mut buf = vec![0u8; 65536];
    
    loop {
        match tokio_socket.recv_from(&mut buf).await {
            Ok((size, addr)) => {
                println!("DEBUG: Received ICMP packet: size={}, from={}", size, addr);
                let icmp_packet = buf[..size].to_vec();
                
                // Parse ICMP packet
                if let Some(embedded_info) = parse_icmp_error(&icmp_packet) {
                    println!("DEBUG: Parsed ICMP error successfully, matching against tracked packets");
                    packet_tracker.match_icmp_error(icmp_packet, embedded_info).await;
                } else {
                    println!("DEBUG: ICMP packet is not an error or failed to parse");
                }
            }
            Err(e) => {
                tracing::error!("ICMP recv error: {}", e);
                println!("DEBUG: ICMP recv error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

/// Parse an ICMP error packet and extract information about the embedded UDP packet
#[cfg(target_os = "linux")]
fn parse_icmp_error(packet: &[u8]) -> Option<EmbeddedUdpInfo> {
    println!("DEBUG: parse_icmp_error called with packet length={}", packet.len());
    
    // ICMP packet structure:
    // 0-19: IP header
    // 20: ICMP type
    // 21: ICMP code
    // 22-23: Checksum
    // 24-27: Rest of ICMP header (varies by type)
    // 28+: Original IP packet that caused the error
    
    if packet.len() < 56 {
        // Need at least: IP(20) + ICMP(8) + embedded IP(20) + embedded UDP(8)
        println!("DEBUG: Packet too small: {} < 56", packet.len());
        return None;
    }
    
    // Check if this is an ICMP error (type 3, 11, or 12)
    let icmp_type = packet[20];
    let icmp_code = packet[21];
    println!("DEBUG: ICMP type={}, code={}", icmp_type, icmp_code);
    
    if ![3, 11, 12].contains(&icmp_type) {
        println!("DEBUG: Not an ICMP error type (expected 3, 11, or 12)");
        return None;
    }
    
    // Extract embedded IP packet (starts at offset 28 in ICMP packet)
    let embedded_ip_start = 28;
    
    // Parse embedded IP header
    let embedded_ip_version = (packet[embedded_ip_start] >> 4) & 0x0F;
    println!("DEBUG: Embedded IP version={}", embedded_ip_version);
    
    if embedded_ip_version != 4 {
        // Only handle IPv4 for now
        println!("DEBUG: Not IPv4 embedded packet");
        return None;
    }
    
    let embedded_ip_header_len = ((packet[embedded_ip_start] & 0x0F) * 4) as usize;
    let embedded_protocol = packet[embedded_ip_start + 9];
    
    println!("DEBUG: Embedded IP header_len={}, protocol={}", embedded_ip_header_len, embedded_protocol);
    
    // Check if embedded packet is UDP (protocol 17)
    if embedded_protocol != 17 {
        println!("DEBUG: Embedded packet is not UDP (protocol 17)");
        return None;
    }
    
    // Extract destination IP from embedded packet
    let dest_ip = Ipv4Addr::new(
        packet[embedded_ip_start + 16],
        packet[embedded_ip_start + 17],
        packet[embedded_ip_start + 18],
        packet[embedded_ip_start + 19],
    );
    
    // Parse embedded UDP header
    let embedded_udp_start = embedded_ip_start + embedded_ip_header_len;
    
    if packet.len() < embedded_udp_start + 8 {
        return None;
    }
    
    let src_port = u16::from_be_bytes([
        packet[embedded_udp_start],
        packet[embedded_udp_start + 1],
    ]);
    
    let dest_port = u16::from_be_bytes([
        packet[embedded_udp_start + 2],
        packet[embedded_udp_start + 3],
    ]);
    
    // Extract UDP length (offset 4-5 in UDP header)
    let udp_length = u16::from_be_bytes([
        packet[embedded_udp_start + 4],
        packet[embedded_udp_start + 5],
    ]);
    
    // Extract first 8 bytes of UDP payload (for matching) - though usually empty in ICMP Time Exceeded
    let payload_start = embedded_udp_start + 8;
    let payload_end = std::cmp::min(payload_start + 8, packet.len());
    let payload_prefix = packet[payload_start..payload_end].to_vec();
    
    println!("DEBUG: Parsed ICMP error successfully:");
    println!("  ICMP type={}, code={}", icmp_type, icmp_code);
    println!("  src_port={}, dest={}:{}", src_port, dest_ip, dest_port);
    println!("  udp_length={}", udp_length);
    println!("  payload_prefix len={}", payload_prefix.len());
    
    tracing::debug!(
        "Parsed ICMP error: type={}, src_port={}, dest={}:{}, udp_length={}",
        icmp_type,
        src_port,
        dest_ip,
        dest_port,
        udp_length
    );
    
    Some(EmbeddedUdpInfo {
        src_port,
        dest_addr: SocketAddr::new(IpAddr::V4(dest_ip), dest_port),
        udp_length,
        payload_prefix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_icmp_parsing_basic() {
        // This is a simplified test - real ICMP packets would be more complex
        // Just verify the function doesn't panic on various inputs
        
        let empty = vec![];
        assert!(parse_icmp_error(&empty).is_none());
        
        let too_small = vec![0u8; 30];
        assert!(parse_icmp_error(&too_small).is_none());
    }
}
