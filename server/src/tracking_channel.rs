/// Global tracking channel for UDP-to-ICMP packet tracking communication
/// 
/// This module provides a global callback that the UDP layer can invoke to
/// send packet tracking information to the ICMP listener, without needing
/// to pass context through multiple layers or create circular dependencies.

use std::sync::OnceLock;
use std::net::SocketAddr;
use std::time::Instant;

/// Callback type for tracking UDP packets
/// Parameters: (dest_addr, src_addr, udp_length, ttl, cleartext_data, sent_at, conn_id, udp_checksum)
pub type TrackingCallback = Box<dyn Fn(SocketAddr, Option<SocketAddr>, u16, Option<u8>, Vec<u8>, Instant, String, u16) + Send + Sync>;

static TRACKING_CALLBACK: OnceLock<TrackingCallback> = OnceLock::new();

/// Initialize the global tracking callback
/// Should be called once at application startup
pub fn init_tracking_callback<F>(callback: F)
where
    F: Fn(SocketAddr, Option<SocketAddr>, u16, Option<u8>, Vec<u8>, Instant, String, u16) + Send + Sync + 'static,
{
    if TRACKING_CALLBACK.set(Box::new(callback)).is_err() {
        panic!("Tracking callback already initialized");
    }
}

/// Track a UDP packet by invoking the global callback
/// This is meant to be called from the UDP sending layer
pub fn track_udp_packet(
    dest_addr: SocketAddr,
    src_addr: Option<SocketAddr>,
    udp_length: u16,
    ttl: Option<u8>,
    cleartext: Vec<u8>,
    sent_at: Instant,
    conn_id: String,
    udp_checksum: u16,
) {
    if let Some(callback) = TRACKING_CALLBACK.get() {
        callback(dest_addr, src_addr, udp_length, ttl, cleartext, sent_at, conn_id, udp_checksum);
    }
}

/// Calculate UDP checksum for IPv4
/// 
/// The UDP checksum includes a pseudo-header with:
/// - Source IP (4 bytes)
/// - Destination IP (4 bytes)
/// - Zero byte (1 byte)
/// - Protocol (1 byte = 17 for UDP)
/// - UDP length (2 bytes)
/// 
/// Followed by the UDP header and payload.
pub fn calculate_udp_checksum_v4(
    src_ip: std::net::Ipv4Addr,
    dest_ip: std::net::Ipv4Addr,
    src_port: u16,
    dest_port: u16,
    payload: &[u8],
) -> u16 {
    let udp_length = (8 + payload.len()) as u16;
    
    // Build pseudo-header + UDP header + payload
    let mut data: Vec<u8> = Vec::with_capacity(12 + 8 + payload.len());
    
    // Pseudo-header
    data.extend_from_slice(&src_ip.octets());        // Source IP (4 bytes)
    data.extend_from_slice(&dest_ip.octets());       // Destination IP (4 bytes)
    data.push(0);                                     // Zero byte
    data.push(17);                                    // Protocol (UDP = 17)
    data.extend_from_slice(&udp_length.to_be_bytes()); // UDP length (2 bytes)
    
    // UDP header
    data.extend_from_slice(&src_port.to_be_bytes()); // Source port (2 bytes)
    data.extend_from_slice(&dest_port.to_be_bytes()); // Destination port (2 bytes)
    data.extend_from_slice(&udp_length.to_be_bytes()); // Length (2 bytes)
    data.extend_from_slice(&[0, 0]);                  // Checksum (initially 0 for calculation)
    
    // UDP payload
    data.extend_from_slice(payload);
    
    // Pad to even length if necessary
    if data.len() % 2 != 0 {
        data.push(0);
    }
    
    // Calculate one's complement sum
    let mut sum: u32 = 0;
    for chunk in data.chunks(2) {
        // Safe indexing: chunk is guaranteed to have 2 bytes after padding
        let word = u16::from_be_bytes([chunk[0], chunk.get(1).copied().unwrap_or(0)]);
        sum = sum.wrapping_add(word as u32);
    }
    
    // Fold 32-bit sum to 16 bits
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    
    // One's complement
    let checksum = !(sum as u16);
    
    // If calculated checksum is 0, return 0xFFFF instead.
    // In UDP, a checksum field of 0 means "no checksum computed" (IPv4 only),
    // so we use 0xFFFF to represent a valid computed checksum that happens to be 0.
    if checksum == 0 {
        0xFFFF
    } else {
        checksum
    }
}

/// Calculate UDP checksum for IPv6
/// 
/// The UDP checksum for IPv6 includes a pseudo-header with:
/// - Source IPv6 address (16 bytes)
/// - Destination IPv6 address (16 bytes)
/// - UDP length (4 bytes, zero-extended)
/// - Zeros (3 bytes)
/// - Next header (1 byte = 17 for UDP)
/// 
/// Note: Unlike IPv4, UDP checksum is mandatory for IPv6.
pub fn calculate_udp_checksum_v6(
    src_ip: std::net::Ipv6Addr,
    dest_ip: std::net::Ipv6Addr,
    src_port: u16,
    dest_port: u16,
    payload: &[u8],
) -> u16 {
    let udp_length = (8 + payload.len()) as u32;
    
    // Build pseudo-header + UDP header + payload
    let mut data: Vec<u8> = Vec::with_capacity(40 + 8 + payload.len());
    
    // Pseudo-header for IPv6
    data.extend_from_slice(&src_ip.octets());         // Source IPv6 (16 bytes)
    data.extend_from_slice(&dest_ip.octets());        // Destination IPv6 (16 bytes)
    data.extend_from_slice(&udp_length.to_be_bytes()); // UDP length (4 bytes)
    data.extend_from_slice(&[0, 0, 0]);               // Zeros (3 bytes)
    data.push(17);                                     // Next header (UDP = 17)
    
    // UDP header
    data.extend_from_slice(&src_port.to_be_bytes());   // Source port (2 bytes)
    data.extend_from_slice(&dest_port.to_be_bytes());  // Destination port (2 bytes)
    data.extend_from_slice(&(udp_length as u16).to_be_bytes()); // Length (2 bytes)
    data.extend_from_slice(&[0, 0]);                   // Checksum (initially 0 for calculation)
    
    // UDP payload
    data.extend_from_slice(payload);
    
    // Pad to even length if necessary
    if data.len() % 2 != 0 {
        data.push(0);
    }
    
    // Calculate one's complement sum
    let mut sum: u32 = 0;
    for chunk in data.chunks(2) {
        // Safe indexing: chunk is guaranteed to have 2 bytes after padding
        let word = u16::from_be_bytes([chunk[0], chunk.get(1).copied().unwrap_or(0)]);
        sum = sum.wrapping_add(word as u32);
    }
    
    // Fold 32-bit sum to 16 bits
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    
    // One's complement
    let checksum = !(sum as u16);
    
    // For IPv6, UDP checksum is mandatory and a computed value of 0 must be
    // transmitted as 0xFFFF (RFC 2460). Unlike IPv4, there's no "no checksum" option.
    if checksum == 0 {
        0xFFFF
    } else {
        checksum
    }
}

/// C-compatible FFI function for tracking IPv4 UDP packets
/// This can be called from the vendored webrtc-util code
#[no_mangle]
pub extern "C" fn wifi_verify_track_udp_packet(
    src_ip_v4: u32,       // Source IPv4 address as u32
    src_port: u16,        // Source port in host byte order
    dest_ip_v4: u32,      // Destination IPv4 address as u32
    dest_port: u16,       // Destination port in host byte order
    udp_length: u16,      // UDP packet length
    ttl: u8,              // TTL value
    buf_ptr: *const u8,   // Pointer to buffer data
    buf_len: usize,       // Buffer length
    conn_id_ptr: *const u8, // Pointer to conn_id string
    conn_id_len: usize,     // conn_id string length
) {
    if buf_ptr.is_null() || buf_len == 0 {
        return;
    }
    
    // Safety: We trust the caller to provide valid pointers
    let cleartext = unsafe {
        std::slice::from_raw_parts(buf_ptr, buf_len).to_vec()
    };
    
    // Extract conn_id from pointer
    let conn_id = if conn_id_ptr.is_null() || conn_id_len == 0 {
        String::new()
    } else {
        unsafe {
            let bytes = std::slice::from_raw_parts(conn_id_ptr, conn_id_len);
            String::from_utf8_lossy(bytes).to_string()
        }
    };
    
    let dest_addr = SocketAddr::from((
        std::net::Ipv4Addr::from(dest_ip_v4),
        dest_port,
    ));

    let src_addr = SocketAddr::from((
        std::net::Ipv4Addr::from(src_ip_v4),
        src_port,
    ));
    let src_addr = Some(src_addr);
    
    // Calculate UDP checksum for IPv4
    let udp_checksum = calculate_udp_checksum_v4(
        std::net::Ipv4Addr::from(src_ip_v4),
        std::net::Ipv4Addr::from(dest_ip_v4),
        src_port,
        dest_port,
        &cleartext,
    );
    
    track_udp_packet(
        dest_addr,
        src_addr,
        udp_length,
        Some(ttl),
        cleartext,
        Instant::now(),
        conn_id,
        udp_checksum,
    );
}

/// C-compatible FFI function for tracking IPv6 UDP packets
/// This can be called from the vendored webrtc-util code
#[no_mangle]
pub extern "C" fn wifi_verify_track_udp_packet_v6(
    src_ip_v6_ptr: *const u8,   // Pointer to 16-byte source IPv6 address
    src_port: u16,              // Source port in host byte order
    dest_ip_v6_ptr: *const u8,  // Pointer to 16-byte destination IPv6 address
    dest_port: u16,             // Destination port in host byte order
    udp_length: u16,            // UDP packet length
    hop_limit: u8,              // IPv6 Hop Limit (equivalent to IPv4 TTL)
    buf_ptr: *const u8,         // Pointer to buffer data
    buf_len: usize,             // Buffer length
    conn_id_ptr: *const u8,     // Pointer to conn_id string
    conn_id_len: usize,         // conn_id string length
) {
    const IPV6_ADDR_LEN: usize = 16;
    
    if src_ip_v6_ptr.is_null() || dest_ip_v6_ptr.is_null() || buf_ptr.is_null() || buf_len == 0 {
        return;
    }
    
    // Safety: We trust the caller to provide valid pointers
    let src_ip_bytes = unsafe {
        let slice = std::slice::from_raw_parts(src_ip_v6_ptr, IPV6_ADDR_LEN);
        let mut arr = [0u8; IPV6_ADDR_LEN];
        arr.copy_from_slice(slice);
        arr
    };
    
    let dest_ip_bytes = unsafe {
        let slice = std::slice::from_raw_parts(dest_ip_v6_ptr, IPV6_ADDR_LEN);
        let mut arr = [0u8; IPV6_ADDR_LEN];
        arr.copy_from_slice(slice);
        arr
    };
    
    let cleartext = unsafe {
        std::slice::from_raw_parts(buf_ptr, buf_len).to_vec()
    };
    
    // Extract conn_id from pointer
    let conn_id = if conn_id_ptr.is_null() || conn_id_len == 0 {
        String::new()
    } else {
        unsafe {
            let bytes = std::slice::from_raw_parts(conn_id_ptr, conn_id_len);
            String::from_utf8_lossy(bytes).to_string()
        }
    };
    
    let dest_addr = SocketAddr::from((
        std::net::Ipv6Addr::from(dest_ip_bytes),
        dest_port,
    ));

    let src_addr = SocketAddr::from((
        std::net::Ipv6Addr::from(src_ip_bytes),
        src_port,
    ));
    let src_addr = Some(src_addr);
    
    // Calculate UDP checksum for IPv6
    let udp_checksum = calculate_udp_checksum_v6(
        std::net::Ipv6Addr::from(src_ip_bytes),
        std::net::Ipv6Addr::from(dest_ip_bytes),
        src_port,
        dest_port,
        &cleartext,
    );
    
    track_udp_packet(
        dest_addr,
        src_addr,
        udp_length,
        Some(hop_limit),
        cleartext,
        Instant::now(),
        conn_id,
        udp_checksum,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_udp_checksum_v4_basic() {
        // Test basic UDP checksum calculation for IPv4
        let src_ip = std::net::Ipv4Addr::new(192, 168, 1, 100);
        let dest_ip = std::net::Ipv4Addr::new(192, 168, 1, 1);
        let src_port = 12345;
        let dest_port = 8080;
        let payload = b"Hello, World!";
        
        let checksum = calculate_udp_checksum_v4(src_ip, dest_ip, src_port, dest_port, payload);
        
        // Checksum should be non-zero
        assert_ne!(checksum, 0, "Checksum should not be zero");
        
        // Same inputs should produce same checksum (deterministic)
        let checksum2 = calculate_udp_checksum_v4(src_ip, dest_ip, src_port, dest_port, payload);
        assert_eq!(checksum, checksum2, "Checksum should be deterministic");
    }
    
    #[test]
    fn test_udp_checksum_v4_different_payload() {
        let src_ip = std::net::Ipv4Addr::new(192, 168, 1, 100);
        let dest_ip = std::net::Ipv4Addr::new(192, 168, 1, 1);
        let src_port = 12345;
        let dest_port = 8080;
        
        let checksum1 = calculate_udp_checksum_v4(src_ip, dest_ip, src_port, dest_port, b"Payload A");
        let checksum2 = calculate_udp_checksum_v4(src_ip, dest_ip, src_port, dest_port, b"Payload B");
        
        // Different payloads should produce different checksums
        assert_ne!(checksum1, checksum2, "Different payloads should produce different checksums");
    }
    
    #[test]
    fn test_udp_checksum_v4_different_ports() {
        let src_ip = std::net::Ipv4Addr::new(192, 168, 1, 100);
        let dest_ip = std::net::Ipv4Addr::new(192, 168, 1, 1);
        let payload = b"Test";
        
        let checksum1 = calculate_udp_checksum_v4(src_ip, dest_ip, 12345, 8080, payload);
        let checksum2 = calculate_udp_checksum_v4(src_ip, dest_ip, 12346, 8080, payload);
        let checksum3 = calculate_udp_checksum_v4(src_ip, dest_ip, 12345, 8081, payload);
        
        // Different ports should produce different checksums
        assert_ne!(checksum1, checksum2, "Different src ports should produce different checksums");
        assert_ne!(checksum1, checksum3, "Different dest ports should produce different checksums");
    }
    
    #[test]
    fn test_udp_checksum_v6_basic() {
        // Test basic UDP checksum calculation for IPv6
        let src_ip = std::net::Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);
        let dest_ip = std::net::Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2);
        let src_port = 12345;
        let dest_port = 8080;
        let payload = b"Hello, IPv6 World!";
        
        let checksum = calculate_udp_checksum_v6(src_ip, dest_ip, src_port, dest_port, payload);
        
        // Checksum should be non-zero
        assert_ne!(checksum, 0, "Checksum should not be zero");
        
        // Same inputs should produce same checksum (deterministic)
        let checksum2 = calculate_udp_checksum_v6(src_ip, dest_ip, src_port, dest_port, payload);
        assert_eq!(checksum, checksum2, "Checksum should be deterministic");
    }
    
    #[test]
    fn test_udp_checksum_v6_different_addresses() {
        let payload = b"Test";
        let src_port = 12345;
        let dest_port = 8080;
        
        let src1 = std::net::Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);
        let src2 = std::net::Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 99);
        let dest = std::net::Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2);
        
        let checksum1 = calculate_udp_checksum_v6(src1, dest, src_port, dest_port, payload);
        let checksum2 = calculate_udp_checksum_v6(src2, dest, src_port, dest_port, payload);
        
        // Different source addresses should produce different checksums
        assert_ne!(checksum1, checksum2, "Different src addresses should produce different checksums");
    }
    
    #[test]
    fn test_udp_checksum_empty_payload() {
        let src_ip = std::net::Ipv4Addr::new(192, 168, 1, 100);
        let dest_ip = std::net::Ipv4Addr::new(192, 168, 1, 1);
        
        // Empty payload should still produce a valid checksum
        let checksum = calculate_udp_checksum_v4(src_ip, dest_ip, 12345, 8080, &[]);
        assert_ne!(checksum, 0, "Empty payload should produce non-zero checksum");
    }
}
