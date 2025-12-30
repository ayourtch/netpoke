/// ICMP error listener for packet tracking correlation
/// 
/// This module listens for ICMP errors using a raw socket and correlates them
/// with tracked UDP packets. Requires CAP_NET_RAW or root privileges.

use std::sync::Arc;
use crate::packet_tracker::{PacketTracker, EmbeddedUdpInfo, MAX_PAYLOAD_PREFIX_SIZE};
use std::net::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr};

// Constants for packet parsing
const IPV6_HEADER_SIZE: usize = 40;
const UDP_HEADER_SIZE: usize = 8;
const ICMPV6_HEADER_SIZE: usize = 8;
const MIN_ICMPV6_PACKET_SIZE: usize = ICMPV6_HEADER_SIZE + IPV6_HEADER_SIZE + UDP_HEADER_SIZE; // 56 bytes

/// Start the ICMP listener in the background
pub fn start_icmp_listener(packet_tracker: Arc<PacketTracker>) {
    #[cfg(target_os = "linux")]
    {
        // Start IPv4 ICMP listener
        let tracker_v4 = packet_tracker.clone();
        tokio::spawn(async move {
            if let Err(e) = icmp_listener_task_v4(tracker_v4).await {
                tracing::error!("IPv4 ICMP listener error: {}", e);
            }
        });
        
        // Start IPv6 ICMPv6 listener
        let tracker_v6 = packet_tracker.clone();
        tokio::spawn(async move {
            if let Err(e) = icmp_listener_task_v6(tracker_v6).await {
                tracing::error!("IPv6 ICMP listener error: {}", e);
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
async fn icmp_listener_task_v4(packet_tracker: Arc<PacketTracker>) -> std::io::Result<()> {
    use socket2::{Socket, Domain, Type, Protocol};
    
    tracing::info!("Starting IPv4 ICMP listener...");
    tracing::debug!("IPv4 ICMP listener starting");
    
    // Create raw ICMPv4 socket
    let socket = match Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)) {
        Ok(s) => {
            tracing::debug!("IPv4 ICMP socket created successfully");
            s
        },
        Err(e) => {
            tracing::error!(
                "Failed to create IPv4 ICMP socket (requires CAP_NET_RAW or root): {}",
                e
            );
            tracing::debug!("Failed to create IPv4 ICMP socket: {}", e);
            return Err(e);
        }
    };
    
    socket.set_nonblocking(true)?;
    
    let std_socket: std::net::UdpSocket = socket.into();
    let tokio_socket = tokio::net::UdpSocket::from_std(std_socket)?;
    
    tracing::info!("IPv4 ICMP listener started successfully");
    tracing::debug!("IPv4 ICMP listener ready to receive packets");
    
    let mut buf = vec![0u8; 65536];
    
    loop {
        match tokio_socket.recv_from(&mut buf).await {
            Ok((size, addr)) => {
                tracing::debug!("Received IPv4 ICMP packet: size={}, from={}", size, addr);
                let icmp_packet = buf[..size].to_vec();
                let router_ip = Some(addr.ip().to_string());
                
                // Parse ICMP packet
                if let Some(embedded_info) = parse_icmp_error(&icmp_packet) {
                    tracing::debug!("Parsed IPv4 ICMP error successfully, matching against tracked packets");
                    packet_tracker.match_icmp_error(icmp_packet, embedded_info, router_ip).await;
                } else {
                    tracing::debug!("IPv4 ICMP packet is not an error or failed to parse");
                }
            }
            Err(e) => {
                tracing::error!("IPv4 ICMP recv error: {}", e);
                tracing::debug!("IPv4 ICMP recv error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

#[cfg(target_os = "linux")]
async fn icmp_listener_task_v6(packet_tracker: Arc<PacketTracker>) -> std::io::Result<()> {
    use socket2::{Socket, Domain, Type, Protocol};
    
    tracing::info!("Starting IPv6 ICMPv6 listener...");
    tracing::debug!("IPv6 ICMPv6 listener starting");
    
    // Create raw ICMPv6 socket
    let socket = match Socket::new(Domain::IPV6, Type::RAW, Some(Protocol::ICMPV6)) {
        Ok(s) => {
            tracing::debug!("IPv6 ICMPv6 socket created successfully");
            s
        },
        Err(e) => {
            tracing::error!(
                "Failed to create IPv6 ICMPv6 socket (requires CAP_NET_RAW or root): {}",
                e
            );
            tracing::debug!("Failed to create IPv6 ICMPv6 socket: {}", e);
            return Err(e);
        }
    };
    
    socket.set_nonblocking(true)?;
    
    let std_socket: std::net::UdpSocket = socket.into();
    let tokio_socket = tokio::net::UdpSocket::from_std(std_socket)?;
    
    tracing::info!("IPv6 ICMPv6 listener started successfully");
    tracing::debug!("IPv6 ICMPv6 listener ready to receive packets");
    
    let mut buf = vec![0u8; 65536];
    
    loop {
        match tokio_socket.recv_from(&mut buf).await {
            Ok((size, addr)) => {
                tracing::debug!("Received IPv6 ICMPv6 packet: size={}, from={}", size, addr);
                let icmp_packet = buf[..size].to_vec();
                let router_ip = Some(addr.ip().to_string());
                
                // Parse ICMPv6 packet
                if let Some(embedded_info) = parse_icmpv6_error(&icmp_packet) {
                    tracing::debug!("Parsed IPv6 ICMPv6 error successfully, matching against tracked packets");
                    packet_tracker.match_icmp_error(icmp_packet, embedded_info, router_ip).await;
                } else {
                    tracing::debug!("IPv6 ICMPv6 packet is not an error or failed to parse");
                }
            }
            Err(e) => {
                tracing::error!("IPv6 ICMPv6 recv error: {}", e);
                tracing::debug!("IPv6 ICMPv6 recv error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

/// Parse an ICMP error packet and extract information about the embedded UDP packet
#[cfg(target_os = "linux")]
fn parse_icmp_error(packet: &[u8]) -> Option<EmbeddedUdpInfo> {
    tracing::debug!("parse_icmp_error called with packet length={}", packet.len());
    
    // ICMP packet structure:
    // 0-19: IP header
    // 20: ICMP type
    // 21: ICMP code
    // 22-23: Checksum
    // 24-27: Rest of ICMP header (varies by type)
    // 28+: Original IP packet that caused the error
    
    if packet.len() < 56 {
        // Need at least: IP(20) + ICMP(8) + embedded IP(20) + embedded UDP(8)
        tracing::debug!("Packet too small: {} < 56", packet.len());
        return None;
    }
    
    // Check if this is an ICMP error (type 3, 11, or 12)
    let icmp_type = packet[20];
    let icmp_code = packet[21];
    tracing::debug!("ICMP type={}, code={}", icmp_type, icmp_code);
    
    if ![3, 11, 12].contains(&icmp_type) {
        tracing::debug!("Not an ICMP error type (expected 3, 11, or 12)");
        return None;
    }
    
    // Extract embedded IP packet (starts at offset 28 in ICMP packet)
    let embedded_ip_start = 28;
    
    // Parse embedded IP header
    let embedded_ip_version = (packet[embedded_ip_start] >> 4) & 0x0F;
    tracing::debug!("Embedded IP version={}", embedded_ip_version);
    
    if embedded_ip_version != 4 {
        // Only handle IPv4 for now
        tracing::debug!("Not IPv4 embedded packet");
        return None;
    }
    
    let embedded_ip_header_len = ((packet[embedded_ip_start] & 0x0F) * 4) as usize;
    let embedded_protocol = packet[embedded_ip_start + 9];
    
    tracing::debug!("Embedded IP header_len={}, protocol={}", embedded_ip_header_len, embedded_protocol);
    
    // Check if embedded packet is UDP (protocol 17)
    if embedded_protocol != 17 {
        tracing::debug!("Embedded packet is not UDP (protocol 17)");
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
    
    // Extract UDP checksum (offset 6-7 in UDP header)
    let udp_checksum = u16::from_be_bytes([
        packet[embedded_udp_start + 6],
        packet[embedded_udp_start + 7],
    ]);
    
    // Extract first 64 bytes of UDP payload (for matching)
    // RFC 792 requires at least 8 bytes of original datagram to be included,
    // but many routers include more. We try to extract up to MAX_PAYLOAD_PREFIX_SIZE bytes.
    let payload_start = embedded_udp_start + 8;
    let payload_end = std::cmp::min(payload_start + MAX_PAYLOAD_PREFIX_SIZE, packet.len());
    let payload_prefix = packet[payload_start..payload_end].to_vec();
    
    tracing::debug!("Parsed ICMP error successfully:");
    tracing::debug!("  ICMP type={}, code={}", icmp_type, icmp_code);
    tracing::debug!("  src_port={}, dest={}:{}", src_port, dest_ip, dest_port);
    tracing::debug!("  udp_length={}, udp_checksum={:#06x}", udp_length, udp_checksum);
    tracing::debug!("  payload_prefix len={}", payload_prefix.len());
    
    tracing::debug!(
        "Parsed ICMP error: type={}, src_port={}, dest={}:{}, udp_length={}, udp_checksum={:#06x}, payload_bytes={}",
        icmp_type,
        src_port,
        dest_ip,
        dest_port,
        udp_length,
        udp_checksum,
        payload_prefix.len()
    );
    
    Some(EmbeddedUdpInfo {
        src_port,
        dest_addr: SocketAddr::new(IpAddr::V4(dest_ip), dest_port),
        udp_length,
        payload_prefix,
        udp_checksum,
    })
}

/// Parse an ICMPv6 error packet and extract information about the embedded UDP packet
/// 
/// ICMPv6 packet structure (RFC 4443):
/// 0: ICMPv6 type (3 = Time Exceeded)
/// 1: ICMPv6 code
/// 2-3: Checksum
/// 4-7: Unused (must be zero)
/// 8+: Original IPv6 packet that caused the error
#[cfg(target_os = "linux")]
fn parse_icmpv6_error(packet: &[u8]) -> Option<EmbeddedUdpInfo> {
    tracing::debug!("parse_icmpv6_error called with packet length={}", packet.len());
    
    // ICMPv6 packet structure (no outer IP header - kernel strips it for raw ICMPv6 sockets):
    // 0: ICMPv6 type
    // 1: ICMPv6 code
    // 2-3: Checksum
    // 4-7: Rest of ICMPv6 header (varies by type)
    // 8+: Original IPv6 packet that caused the error
    
    if packet.len() < MIN_ICMPV6_PACKET_SIZE {
        tracing::debug!("ICMPv6 packet too small: {} < {}", packet.len(), MIN_ICMPV6_PACKET_SIZE);
        return None;
    }
    
    // Check if this is an ICMPv6 error (type 1=Destination Unreachable, 3=Time Exceeded)
    let icmpv6_type = packet[0];
    let icmpv6_code = packet[1];
    tracing::debug!("ICMPv6 type={}, code={}", icmpv6_type, icmpv6_code);
    
    // ICMPv6 Time Exceeded = type 3
    // ICMPv6 Destination Unreachable = type 1
    // ICMPv6 Packet Too Big = type 2
    if ![1, 2, 3].contains(&icmpv6_type) {
        tracing::debug!("Not an ICMPv6 error type (expected 1, 2, or 3)");
        return None;
    }
    
    // Extract embedded IPv6 packet (starts at offset 8 in ICMPv6 packet)
    let embedded_ip_start = ICMPV6_HEADER_SIZE;
    
    if packet.len() < embedded_ip_start + IPV6_HEADER_SIZE {
        tracing::debug!("Not enough data for embedded IPv6 header");
        return None;
    }
    
    // Parse embedded IPv6 header
    let embedded_ip_version = (packet[embedded_ip_start] >> 4) & 0x0F;
    tracing::debug!("Embedded IP version={}", embedded_ip_version);
    
    if embedded_ip_version != 6 {
        tracing::debug!("Not IPv6 embedded packet");
        return None;
    }
    
    // IPv6 header is fixed 40 bytes
    // Next header field is at offset 6
    let next_header = packet[embedded_ip_start + 6];
    tracing::debug!("Embedded IPv6 next_header={}", next_header);
    
    // Check if embedded packet is UDP (next header 17)
    if next_header != 17 {
        tracing::debug!("Embedded packet is not UDP (next header 17)");
        return None;
    }
    
    // Extract destination IPv6 address from embedded packet (bytes 24-39 of IPv6 header)
    let dest_ip_bytes: [u8; 16] = packet[embedded_ip_start + 24..embedded_ip_start + 40]
        .try_into()
        .ok()?;
    let dest_ip = Ipv6Addr::from(dest_ip_bytes);
    
    // Parse embedded UDP header (starts after IPv6 header)
    let embedded_udp_start = embedded_ip_start + IPV6_HEADER_SIZE;
    
    if packet.len() < embedded_udp_start + UDP_HEADER_SIZE {
        tracing::debug!("Not enough data for embedded UDP header");
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
    
    // Extract UDP checksum (offset 6-7 in UDP header)
    let udp_checksum = u16::from_be_bytes([
        packet[embedded_udp_start + 6],
        packet[embedded_udp_start + 7],
    ]);
    
    // Extract first 64 bytes of UDP payload (for matching)
    // RFC 4443 requires at least 1280 bytes of the original packet to be included,
    // so we should have enough data to extract the full payload prefix.
    let payload_start = embedded_udp_start + UDP_HEADER_SIZE;
    let payload_end = std::cmp::min(payload_start + MAX_PAYLOAD_PREFIX_SIZE, packet.len());
    let payload_prefix = packet[payload_start..payload_end].to_vec();
    
    tracing::debug!("Parsed ICMPv6 error successfully:");
    tracing::debug!("  ICMPv6 type={}, code={}", icmpv6_type, icmpv6_code);
    tracing::debug!("  src_port={}, dest=[{}]:{}", src_port, dest_ip, dest_port);
    tracing::debug!("  udp_length={}, udp_checksum={:#06x}", udp_length, udp_checksum);
    tracing::debug!("  payload_prefix len={}", payload_prefix.len());
    
    tracing::debug!(
        "Parsed ICMPv6 error: type={}, src_port={}, dest=[{}]:{}, udp_length={}, udp_checksum={:#06x}, payload_bytes={}",
        icmpv6_type,
        src_port,
        dest_ip,
        dest_port,
        udp_length,
        udp_checksum,
        payload_prefix.len()
    );
    
    Some(EmbeddedUdpInfo {
        src_port,
        dest_addr: SocketAddr::new(IpAddr::V6(dest_ip), dest_port),
        udp_length,
        payload_prefix,
        udp_checksum,
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
    
    #[test]
    fn test_icmpv6_parsing_basic() {
        // This is a simplified test - real ICMPv6 packets would be more complex
        // Just verify the function doesn't panic on various inputs
        
        let empty = vec![];
        assert!(parse_icmpv6_error(&empty).is_none());
        
        let too_small = vec![0u8; 30];
        assert!(parse_icmpv6_error(&too_small).is_none());
    }
}
