/// Global tracking channel for UDP-to-ICMP packet tracking communication
/// 
/// This module provides a global callback that the UDP layer can invoke to
/// send packet tracking information to the ICMP listener, without needing
/// to pass context through multiple layers or create circular dependencies.

use std::sync::OnceLock;
use std::net::SocketAddr;
use std::time::Instant;

/// Callback type for tracking UDP packets
/// Parameters: (dest_addr, udp_length, ttl, cleartext_data, sent_at)
pub type TrackingCallback = Box<dyn Fn(SocketAddr, u16, Option<u8>, Vec<u8>, Instant) + Send + Sync>;

static TRACKING_CALLBACK: OnceLock<TrackingCallback> = OnceLock::new();

/// Initialize the global tracking callback
/// Should be called once at application startup
pub fn init_tracking_callback<F>(callback: F)
where
    F: Fn(SocketAddr, u16, Option<u8>, Vec<u8>, Instant) + Send + Sync + 'static,
{
    if TRACKING_CALLBACK.set(Box::new(callback)).is_err() {
        panic!("Tracking callback already initialized");
    }
}

/// Track a UDP packet by invoking the global callback
/// This is meant to be called from the UDP sending layer
pub fn track_udp_packet(
    dest_addr: SocketAddr,
    udp_length: u16,
    ttl: Option<u8>,
    cleartext: Vec<u8>,
    sent_at: Instant,
) {
    if let Some(callback) = TRACKING_CALLBACK.get() {
        callback(dest_addr, udp_length, ttl, cleartext, sent_at);
    }
}

/// C-compatible FFI function for tracking UDP packets
/// This can be called from the vendored webrtc-util code
#[no_mangle]
pub extern "C" fn wifi_verify_track_udp_packet(
    dest_ip_v4: u32,      // IPv4 address in network byte order
    dest_port: u16,       // Port in host byte order
    udp_length: u16,      // UDP packet length
    ttl: u8,              // TTL value
    buf_ptr: *const u8,   // Pointer to buffer data
    buf_len: usize,       // Buffer length
) {
    if buf_ptr.is_null() || buf_len == 0 {
        return;
    }
    
    // Safety: We trust the caller to provide valid pointers
    let cleartext = unsafe {
        std::slice::from_raw_parts(buf_ptr, buf_len).to_vec()
    };
    
    let dest_addr = SocketAddr::from((
        std::net::Ipv4Addr::from(u32::from_be(dest_ip_v4)),
        dest_port,
    ));
    
    track_udp_packet(
        dest_addr,
        udp_length,
        Some(ttl),
        cleartext,
        Instant::now(),
    );
}
