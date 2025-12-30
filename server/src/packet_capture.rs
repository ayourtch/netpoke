/// Packet capture module for continuous traffic capture into a ring buffer
///
/// This module provides tcpdump-like packet capture functionality with:
/// - Configurable ring buffer size
/// - Thread-safe access via parking_lot RwLock
/// - PCAP file export capability

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;

/// Captured packet with timestamp
#[derive(Clone)]
pub struct CapturedPacket {
    /// Timestamp when the packet was captured (microseconds since Unix epoch)
    pub timestamp_us: u64,
    /// Original packet length (may be larger than stored data if truncated)
    pub original_len: u32,
    /// Captured packet data (may be truncated)
    pub data: Vec<u8>,
}

/// Configuration for packet capture
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct CaptureConfig {
    /// Maximum number of packets to store in the ring buffer
    pub max_packets: usize,
    /// Maximum bytes per packet to capture (packets larger than this are truncated)
    pub snaplen: u32,
    /// Network interface to capture on (empty string means all interfaces)
    pub interface: String,
    /// Enable packet capture
    pub enabled: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            max_packets: 10000,  // Store up to 10k packets
            snaplen: 65535,      // Full packet capture by default
            interface: String::new(), // All interfaces
            enabled: false,      // Disabled by default
        }
    }
}

/// Ring buffer for storing captured packets
pub struct PacketRingBuffer {
    /// Configuration
    config: CaptureConfig,
    /// Ring buffer of packets
    packets: Vec<CapturedPacket>,
    /// Write position in the ring buffer
    write_pos: usize,
    /// Total number of packets captured (may be > max_packets if wrapped)
    total_captured: u64,
}

impl PacketRingBuffer {
    /// Create a new ring buffer with the given configuration
    pub fn new(config: CaptureConfig) -> Self {
        Self {
            config,
            packets: Vec::new(),
            write_pos: 0,
            total_captured: 0,
        }
    }

    /// Add a packet to the ring buffer
    pub fn add_packet(&mut self, data: Vec<u8>, original_len: u32) {
        let timestamp_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_micros() as u64;

        // Truncate data if necessary
        let truncated_data = if data.len() > self.config.snaplen as usize {
            data[..self.config.snaplen as usize].to_vec()
        } else {
            data
        };

        let packet = CapturedPacket {
            timestamp_us,
            original_len,
            data: truncated_data,
        };

        if self.packets.len() < self.config.max_packets {
            // Buffer not full yet, just append
            self.packets.push(packet);
        } else {
            // Buffer full, overwrite oldest packet
            self.packets[self.write_pos] = packet;
        }

        self.write_pos = (self.write_pos + 1) % self.config.max_packets;
        self.total_captured += 1;
    }

    /// Get all packets in chronological order
    pub fn get_packets(&self) -> Vec<CapturedPacket> {
        if self.packets.len() < self.config.max_packets {
            // Buffer not full, packets are already in order
            self.packets.clone()
        } else {
            // Buffer full, need to reorder from write_pos
            let mut result = Vec::with_capacity(self.config.max_packets);
            result.extend_from_slice(&self.packets[self.write_pos..]);
            result.extend_from_slice(&self.packets[..self.write_pos]);
            result
        }
    }

    /// Get capture statistics
    pub fn stats(&self) -> CaptureStats {
        CaptureStats {
            packets_in_buffer: self.packets.len(),
            max_packets: self.config.max_packets,
            total_captured: self.total_captured,
            snaplen: self.config.snaplen,
        }
    }

    /// Clear all packets from the buffer
    pub fn clear(&mut self) {
        self.packets.clear();
        self.write_pos = 0;
    }
}

/// Statistics about packet capture
#[derive(Clone, Debug, serde::Serialize)]
pub struct CaptureStats {
    /// Number of packets currently in the buffer
    pub packets_in_buffer: usize,
    /// Maximum capacity of the buffer
    pub max_packets: usize,
    /// Total number of packets captured since start
    pub total_captured: u64,
    /// Snapshot length (max bytes per packet)
    pub snaplen: u32,
}

/// Thread-safe packet capture service
pub struct PacketCaptureService {
    /// Ring buffer protected by RwLock
    buffer: RwLock<PacketRingBuffer>,
    /// Configuration
    config: CaptureConfig,
}

impl PacketCaptureService {
    /// Create a new packet capture service
    pub fn new(config: CaptureConfig) -> Arc<Self> {
        Arc::new(Self {
            buffer: RwLock::new(PacketRingBuffer::new(config.clone())),
            config,
        })
    }

    /// Check if capture is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Add a captured packet
    pub fn add_packet(&self, data: Vec<u8>, original_len: u32) {
        if self.config.enabled {
            self.buffer.write().add_packet(data, original_len);
        }
    }

    /// Get all captured packets in chronological order
    pub fn get_packets(&self) -> Vec<CapturedPacket> {
        self.buffer.read().get_packets()
    }

    /// Get capture statistics
    pub fn stats(&self) -> CaptureStats {
        self.buffer.read().stats()
    }

    /// Clear all captured packets
    pub fn clear(&self) {
        self.buffer.write().clear();
    }

    /// Generate PCAP file contents from captured packets
    /// Returns the raw bytes of a valid PCAP file
    pub fn generate_pcap(&self) -> Vec<u8> {
        let packets = self.get_packets();
        let mut output = Vec::new();

        // PCAP file header (24 bytes)
        // Magic number: 0xa1b2c3d4 (microsecond timestamps)
        output.extend_from_slice(&0xa1b2c3d4u32.to_le_bytes());
        // Version major: 2
        output.extend_from_slice(&2u16.to_le_bytes());
        // Version minor: 4
        output.extend_from_slice(&4u16.to_le_bytes());
        // Timezone offset (GMT)
        output.extend_from_slice(&0i32.to_le_bytes());
        // Timestamp accuracy
        output.extend_from_slice(&0u32.to_le_bytes());
        // Snapshot length
        output.extend_from_slice(&self.config.snaplen.to_le_bytes());
        // Link-layer header type: DLT_RAW (101) - raw IP packets
        // Using DLT_EN10MB (1) for Ethernet frames
        output.extend_from_slice(&1u32.to_le_bytes());

        // Write each packet
        for packet in packets {
            // Packet header (16 bytes)
            let ts_sec = (packet.timestamp_us / 1_000_000) as u32;
            let ts_usec = (packet.timestamp_us % 1_000_000) as u32;
            let incl_len = packet.data.len() as u32;
            let orig_len = packet.original_len;

            output.extend_from_slice(&ts_sec.to_le_bytes());
            output.extend_from_slice(&ts_usec.to_le_bytes());
            output.extend_from_slice(&incl_len.to_le_bytes());
            output.extend_from_slice(&orig_len.to_le_bytes());
            
            // Packet data
            output.extend_from_slice(&packet.data);
        }

        output
    }
}

/// Start the packet capture background thread
/// This uses raw sockets to capture all traffic
#[cfg(target_os = "linux")]
pub fn start_packet_capture(service: Arc<PacketCaptureService>) {
    if !service.is_enabled() {
        tracing::info!("Packet capture is disabled");
        return;
    }

    tracing::info!("Starting packet capture service...");

    // Start IPv4 capture thread
    let service_v4 = service.clone();
    tokio::spawn(async move {
        if let Err(e) = capture_loop_v4(service_v4).await {
            tracing::error!("IPv4 packet capture error: {}", e);
        }
    });

    // Start IPv6 capture thread  
    let service_v6 = service.clone();
    tokio::spawn(async move {
        if let Err(e) = capture_loop_v6(service_v6).await {
            tracing::error!("IPv6 packet capture error: {}", e);
        }
    });

    tracing::info!("Packet capture service started");
}

#[cfg(target_os = "linux")]
async fn capture_loop_v4(service: Arc<PacketCaptureService>) -> std::io::Result<()> {
    use socket2::{Socket, Domain, Type};

    // Create raw socket for all IP traffic
    // ETH_P_IP = 0x0800 in network byte order
    let socket = match Socket::new(
        Domain::PACKET,
        Type::RAW,
        Some(socket2::Protocol::from(libc::ETH_P_IP.to_be() as i32)),
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create packet capture socket (requires CAP_NET_RAW): {}", e);
            return Err(e);
        }
    };

    socket.set_nonblocking(true)?;

    // Convert to tokio socket
    let std_socket: std::net::UdpSocket = socket.into();
    let tokio_socket = tokio::net::UdpSocket::from_std(std_socket)?;

    tracing::info!("IPv4 packet capture started");

    let mut buf = vec![0u8; 65536];
    
    loop {
        match tokio_socket.recv(&mut buf).await {
            Ok(size) => {
                if size > 0 {
                    let packet_data = buf[..size].to_vec();
                    service.add_packet(packet_data, size as u32);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available, continue
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            Err(e) => {
                tracing::error!("IPv4 capture recv error: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

#[cfg(target_os = "linux")]
async fn capture_loop_v6(service: Arc<PacketCaptureService>) -> std::io::Result<()> {
    use socket2::{Socket, Domain, Type};

    // Create raw socket for all IPv6 traffic
    // ETH_P_IPV6 = 0x86DD in network byte order
    let socket = match Socket::new(
        Domain::PACKET,
        Type::RAW,
        Some(socket2::Protocol::from(libc::ETH_P_IPV6.to_be() as i32)),
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to create IPv6 packet capture socket (requires CAP_NET_RAW): {}", e);
            return Err(e);
        }
    };

    socket.set_nonblocking(true)?;

    // Convert to tokio socket
    let std_socket: std::net::UdpSocket = socket.into();
    let tokio_socket = tokio::net::UdpSocket::from_std(std_socket)?;

    tracing::info!("IPv6 packet capture started");

    let mut buf = vec![0u8; 65536];
    
    loop {
        match tokio_socket.recv(&mut buf).await {
            Ok(size) => {
                if size > 0 {
                    let packet_data = buf[..size].to_vec();
                    service.add_packet(packet_data, size as u32);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available, continue
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            Err(e) => {
                tracing::error!("IPv6 capture recv error: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub fn start_packet_capture(service: Arc<PacketCaptureService>) {
    if service.is_enabled() {
        tracing::warn!("Packet capture is not implemented for this platform");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_basic() {
        let config = CaptureConfig {
            max_packets: 3,
            snaplen: 100,
            ..Default::default()
        };
        let mut buffer = PacketRingBuffer::new(config);

        // Add 3 packets
        buffer.add_packet(vec![1, 2, 3], 3);
        buffer.add_packet(vec![4, 5, 6], 3);
        buffer.add_packet(vec![7, 8, 9], 3);

        let packets = buffer.get_packets();
        assert_eq!(packets.len(), 3);
        assert_eq!(packets[0].data, vec![1, 2, 3]);
        assert_eq!(packets[1].data, vec![4, 5, 6]);
        assert_eq!(packets[2].data, vec![7, 8, 9]);
    }

    #[test]
    fn test_ring_buffer_wrap() {
        let config = CaptureConfig {
            max_packets: 3,
            snaplen: 100,
            ..Default::default()
        };
        let mut buffer = PacketRingBuffer::new(config);

        // Add 5 packets (wraps around)
        buffer.add_packet(vec![1], 1);
        buffer.add_packet(vec![2], 1);
        buffer.add_packet(vec![3], 1);
        buffer.add_packet(vec![4], 1);
        buffer.add_packet(vec![5], 1);

        let packets = buffer.get_packets();
        assert_eq!(packets.len(), 3);
        // Should have packets 3, 4, 5 in order
        assert_eq!(packets[0].data, vec![3]);
        assert_eq!(packets[1].data, vec![4]);
        assert_eq!(packets[2].data, vec![5]);
    }

    #[test]
    fn test_truncation() {
        let config = CaptureConfig {
            max_packets: 10,
            snaplen: 5,
            ..Default::default()
        };
        let mut buffer = PacketRingBuffer::new(config);

        buffer.add_packet(vec![1, 2, 3, 4, 5, 6, 7, 8], 8);
        let packets = buffer.get_packets();
        assert_eq!(packets[0].data, vec![1, 2, 3, 4, 5]);
        assert_eq!(packets[0].original_len, 8);
    }

    #[test]
    fn test_pcap_generation() {
        let config = CaptureConfig {
            max_packets: 10,
            snaplen: 65535,
            enabled: true,
            ..Default::default()
        };
        let service = PacketCaptureService::new(config);

        // Add a test packet
        service.add_packet(vec![0x45, 0x00, 0x00, 0x20], 4);

        let pcap = service.generate_pcap();
        
        // Verify PCAP header
        assert!(pcap.len() >= 24 + 16 + 4); // header + packet header + packet data
        
        // Check magic number
        let magic = u32::from_le_bytes([pcap[0], pcap[1], pcap[2], pcap[3]]);
        assert_eq!(magic, 0xa1b2c3d4);
    }
}
