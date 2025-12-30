/// Packet capture module using libpcap for tcpdump-like traffic capture
///
/// This module provides packet capture functionality using libpcap, the same
/// library used by tcpdump, Wireshark, and other network analysis tools.
/// Features:
/// - Configurable ring buffer size for storing captured packets
/// - Thread-safe access via parking_lot RwLock
/// - PCAP file export using libpcap's native format
/// - Support for interface selection and promiscuous mode

use std::sync::Arc;
use parking_lot::RwLock;

/// Captured packet with timestamp and metadata from libpcap
#[derive(Clone)]
pub struct CapturedPacket {
    /// Timestamp when the packet was captured (seconds since Unix epoch)
    pub ts_sec: i64,
    /// Microseconds part of the timestamp
    pub ts_usec: i64,
    /// Original packet length on the wire
    pub orig_len: u32,
    /// Captured packet data (may be truncated to snaplen)
    pub data: Vec<u8>,
}

/// Configuration for packet capture
#[derive(Clone, Debug)]
pub struct CaptureConfig {
    /// Maximum number of packets to store in the ring buffer
    pub max_packets: usize,
    /// Maximum bytes per packet to capture (packets larger than this are truncated)
    pub snaplen: i32,
    /// Network interface to capture on (empty string means first available, "any" for all)
    pub interface: String,
    /// Enable packet capture
    pub enabled: bool,
    /// Enable promiscuous mode
    pub promiscuous: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            max_packets: 10000,   // Store up to 10k packets
            snaplen: 65535,       // Full packet capture by default
            interface: String::new(), // First available interface
            enabled: false,       // Disabled by default
            promiscuous: true,    // Promiscuous mode by default
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
    /// Data link type from pcap (needed for PCAP file header)
    datalink: i32,
}

impl PacketRingBuffer {
    /// Create a new ring buffer with the given configuration
    pub fn new(config: CaptureConfig) -> Self {
        Self {
            config,
            packets: Vec::new(),
            write_pos: 0,
            total_captured: 0,
            datalink: 1, // DLT_EN10MB (Ethernet) as default
        }
    }

    /// Set the data link type (called when capture starts)
    pub fn set_datalink(&mut self, datalink: i32) {
        self.datalink = datalink;
    }

    /// Add a packet to the ring buffer
    pub fn add_packet(&mut self, ts_sec: i64, ts_usec: i64, orig_len: u32, data: Vec<u8>) {
        let packet = CapturedPacket {
            ts_sec,
            ts_usec,
            orig_len,
            data,
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
            snaplen: self.config.snaplen as u32,
        }
    }

    /// Clear all packets from the buffer
    pub fn clear(&mut self) {
        self.packets.clear();
        self.write_pos = 0;
    }

    /// Get datalink type
    pub fn datalink(&self) -> i32 {
        self.datalink
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

    /// Get the configured interface
    pub fn interface(&self) -> &str {
        &self.config.interface
    }

    /// Get the snaplen
    pub fn snaplen(&self) -> i32 {
        self.config.snaplen
    }

    /// Get promiscuous mode setting
    pub fn promiscuous(&self) -> bool {
        self.config.promiscuous
    }

    /// Set the data link type (called when capture starts)
    pub fn set_datalink(&self, datalink: i32) {
        self.buffer.write().set_datalink(datalink);
    }

    /// Add a captured packet (called from capture thread)
    pub fn add_packet(&self, ts_sec: i64, ts_usec: i64, orig_len: u32, data: Vec<u8>) {
        if self.config.enabled {
            self.buffer.write().add_packet(ts_sec, ts_usec, orig_len, data);
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
        let buffer = self.buffer.read();
        let packets = buffer.get_packets();
        let datalink = buffer.datalink();
        let snaplen = self.config.snaplen as u32;
        drop(buffer);

        let mut output = Vec::new();

        // PCAP file header (24 bytes) - same format as tcpdump/libpcap
        // Magic number: 0xa1b2c3d4 (microsecond timestamps, little-endian)
        output.extend_from_slice(&0xa1b2c3d4u32.to_le_bytes());
        // Version major: 2
        output.extend_from_slice(&2u16.to_le_bytes());
        // Version minor: 4
        output.extend_from_slice(&4u16.to_le_bytes());
        // Timezone offset (GMT) - always 0 for modern captures
        output.extend_from_slice(&0i32.to_le_bytes());
        // Timestamp accuracy - always 0
        output.extend_from_slice(&0u32.to_le_bytes());
        // Snapshot length
        output.extend_from_slice(&snaplen.to_le_bytes());
        // Link-layer header type (from libpcap)
        output.extend_from_slice(&(datalink as u32).to_le_bytes());

        // Write each packet record
        for packet in packets {
            // Packet record header (16 bytes)
            output.extend_from_slice(&(packet.ts_sec as u32).to_le_bytes());
            output.extend_from_slice(&(packet.ts_usec as u32).to_le_bytes());
            output.extend_from_slice(&(packet.data.len() as u32).to_le_bytes());
            output.extend_from_slice(&packet.orig_len.to_le_bytes());
            
            // Packet data
            output.extend_from_slice(&packet.data);
        }

        output
    }
}

/// Start the packet capture using libpcap
/// This captures packets exactly like tcpdump does
pub fn start_packet_capture(service: Arc<PacketCaptureService>) {
    if !service.is_enabled() {
        tracing::info!("Packet capture is disabled");
        return;
    }

    tracing::info!("Starting packet capture service with libpcap...");

    // Spawn blocking task for pcap capture (pcap is not async-friendly)
    let service_clone = service.clone();
    std::thread::spawn(move || {
        if let Err(e) = run_pcap_capture(service_clone) {
            tracing::error!("Packet capture error: {}", e);
        }
    });
}

/// Run the libpcap capture loop in a blocking thread
fn run_pcap_capture(service: Arc<PacketCaptureService>) -> Result<(), pcap::Error> {
    use pcap::{Capture, Device};

    // Determine which device to capture on
    let device = if service.interface().is_empty() {
        // Use the default device
        Device::lookup()?.ok_or_else(|| pcap::Error::PcapError("No capture device found".into()))?
    } else if service.interface() == "any" {
        // Use the "any" pseudo-device on Linux
        Device {
            name: "any".to_string(),
            desc: Some("Pseudo-device that captures on all interfaces".to_string()),
            addresses: vec![],
            flags: pcap::DeviceFlags::empty(),
        }
    } else {
        // Use the specified interface
        Device {
            name: service.interface().to_string(),
            desc: None,
            addresses: vec![],
            flags: pcap::DeviceFlags::empty(),
        }
    };

    tracing::info!("Opening capture on device: {} ({:?})", device.name, device.desc);

    // Open the capture with libpcap
    let mut cap = Capture::from_device(device)?
        .snaplen(service.snaplen())
        .promisc(service.promiscuous())
        .timeout(1000) // 1 second timeout for periodic checking
        .open()?;

    // Get the data link type and store it for PCAP file generation
    let datalink = cap.get_datalink();
    service.set_datalink(datalink.0);
    tracing::info!("Capture started with datalink type: {:?}", datalink);

    // Capture loop
    loop {
        match cap.next_packet() {
            Ok(packet) => {
                // Extract timestamp from pcap packet header
                let ts_sec = packet.header.ts.tv_sec;
                let ts_usec = packet.header.ts.tv_usec;
                let orig_len = packet.header.len;
                let data = packet.data.to_vec();

                service.add_packet(ts_sec, ts_usec, orig_len, data);
            }
            Err(pcap::Error::TimeoutExpired) => {
                // Timeout is normal, just continue
                continue;
            }
            Err(e) => {
                tracing::error!("Capture error: {}", e);
                // Sleep briefly before retrying on error
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
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
        buffer.add_packet(1000, 0, 3, vec![1, 2, 3]);
        buffer.add_packet(1001, 0, 3, vec![4, 5, 6]);
        buffer.add_packet(1002, 0, 3, vec![7, 8, 9]);

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
        buffer.add_packet(1000, 0, 1, vec![1]);
        buffer.add_packet(1001, 0, 1, vec![2]);
        buffer.add_packet(1002, 0, 1, vec![3]);
        buffer.add_packet(1003, 0, 1, vec![4]);
        buffer.add_packet(1004, 0, 1, vec![5]);

        let packets = buffer.get_packets();
        assert_eq!(packets.len(), 3);
        // Should have packets 3, 4, 5 in order
        assert_eq!(packets[0].data, vec![3]);
        assert_eq!(packets[1].data, vec![4]);
        assert_eq!(packets[2].data, vec![5]);
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

        // Set datalink type (Ethernet)
        service.set_datalink(1);

        // Add a test packet
        service.add_packet(1700000000, 123456, 4, vec![0x45, 0x00, 0x00, 0x20]);

        let pcap = service.generate_pcap();
        
        // Verify PCAP header
        assert!(pcap.len() >= 24 + 16 + 4); // header + packet header + packet data
        
        // Check magic number
        let magic = u32::from_le_bytes([pcap[0], pcap[1], pcap[2], pcap[3]]);
        assert_eq!(magic, 0xa1b2c3d4);

        // Check version
        let version_major = u16::from_le_bytes([pcap[4], pcap[5]]);
        let version_minor = u16::from_le_bytes([pcap[6], pcap[7]]);
        assert_eq!(version_major, 2);
        assert_eq!(version_minor, 4);

        // Check datalink type
        let datalink = u32::from_le_bytes([pcap[20], pcap[21], pcap[22], pcap[23]]);
        assert_eq!(datalink, 1); // DLT_EN10MB
    }
}
