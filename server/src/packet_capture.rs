/// Packet capture module using libpcap for tcpdump-like traffic capture
///
/// This module provides packet capture functionality using libpcap, the same
/// library used by tcpdump, Wireshark, and other network analysis tools.
/// Features:
/// - Configurable ring buffer size for storing captured packets
/// - Thread-safe access via parking_lot RwLock
/// - PCAP file export using libpcap's native format
/// - Support for interface selection and promiscuous mode
/// - Survey session tagging for per-session packet downloads

use std::sync::Arc;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
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
    /// Optional survey session ID that this packet belongs to
    pub survey_session_id: Option<String>,
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
    pub fn add_packet(&mut self, ts_sec: i64, ts_usec: i64, orig_len: u32, data: Vec<u8>, survey_session_id: Option<String>) {
        let packet = CapturedPacket {
            ts_sec,
            ts_usec,
            orig_len,
            data,
            survey_session_id,
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
    
    /// Get packets filtered by survey session ID
    pub fn get_packets_for_session(&self, survey_session_id: &str) -> Vec<CapturedPacket> {
        let all_packets = self.get_packets();
        all_packets
            .into_iter()
            .filter(|p| p.survey_session_id.as_deref() == Some(survey_session_id))
            .collect()
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
        // Note: total_captured is intentionally NOT reset, as it represents
        // the total packets captured since the service started, not since last clear.
        // This is useful for monitoring packet throughput over the lifetime of the service.
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

/// Registry for mapping socket addresses to survey session IDs
/// This allows tagging captured packets with their survey session
#[derive(Default)]
pub struct SessionRegistry {
    /// Map of (client_ip, client_port) -> survey_session_id
    /// This maps client-side addresses to survey sessions
    address_to_session: HashMap<SocketAddr, String>,
    /// Map of server port to survey_session_id for cases where we only know the server port
    server_port_to_session: HashMap<u16, Vec<String>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Register a client address with a survey session ID.
    /// 
    /// The server_port is used as a fallback for ICMP packet matching where
    /// only the destination port is available from the embedded packet.
    /// When server_port is 0, only client address lookup is available.
    pub fn register(&mut self, client_addr: SocketAddr, server_port: u16, survey_session_id: String) {
        tracing::debug!(
            "Registering session: client={}, server_port={}, session_id={}",
            client_addr, server_port, survey_session_id
        );
        self.address_to_session.insert(client_addr, survey_session_id.clone());
        
        // Also register by server port for ICMP matching (skip if port is 0)
        if server_port > 0 {
            self.server_port_to_session
                .entry(server_port)
                .or_insert_with(Vec::new)
                .push(survey_session_id);
        }
    }
    
    /// Unregister a client address
    pub fn unregister(&mut self, client_addr: &SocketAddr) {
        self.address_to_session.remove(client_addr);
    }
    
    /// Look up survey session ID by client address
    pub fn lookup(&self, addr: &SocketAddr) -> Option<&String> {
        self.address_to_session.get(addr)
    }
    
    /// Look up survey session ID by either source or destination address
    /// Returns the first match found
    pub fn lookup_by_either(&self, src: &SocketAddr, dst: &SocketAddr) -> Option<&String> {
        self.address_to_session.get(src)
            .or_else(|| self.address_to_session.get(dst))
    }
    
    /// Look up by server port (for ICMP packets where we only have the embedded destination).
    /// 
    /// Returns the first registered session for this port. When multiple sessions
    /// share a port, this is a best-effort match. The primary lookup should use
    /// client addresses; port-based lookup is a fallback for ICMP error packets.
    pub fn lookup_by_server_port(&self, port: u16) -> Option<&String> {
        self.server_port_to_session.get(&port).and_then(|v| v.first())
    }
}

/// Thread-safe packet capture service
pub struct PacketCaptureService {
    /// Ring buffer protected by RwLock
    buffer: RwLock<PacketRingBuffer>,
    /// Configuration
    config: CaptureConfig,
    /// Session registry for mapping addresses to survey session IDs
    session_registry: RwLock<SessionRegistry>,
}

impl PacketCaptureService {
    /// Create a new packet capture service
    pub fn new(config: CaptureConfig) -> Arc<Self> {
        Arc::new(Self {
            buffer: RwLock::new(PacketRingBuffer::new(config.clone())),
            config,
            session_registry: RwLock::new(SessionRegistry::new()),
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
    
    /// Register a session with the capture service
    /// This allows the capture to tag packets with the survey session ID
    pub fn register_session(&self, client_addr: SocketAddr, server_port: u16, survey_session_id: String) {
        self.session_registry.write().register(client_addr, server_port, survey_session_id);
    }
    
    /// Unregister a session from the capture service
    pub fn unregister_session(&self, client_addr: &SocketAddr) {
        self.session_registry.write().unregister(client_addr);
    }

    /// Add a captured packet (called from capture thread)
    /// The packet will be tagged with a survey session ID if one can be determined
    pub fn add_packet(&self, ts_sec: i64, ts_usec: i64, orig_len: u32, data: Vec<u8>) {
        if self.config.enabled {
            // Try to extract addresses from packet and find matching session
            let survey_session_id = self.extract_session_id_from_packet(&data);
            self.buffer.write().add_packet(ts_sec, ts_usec, orig_len, data, survey_session_id);
        }
    }
    
    /// Extract survey session ID from a captured packet
    /// Parses the packet to find source/destination addresses and looks up in registry
    fn extract_session_id_from_packet(&self, data: &[u8]) -> Option<String> {
        // Parse Ethernet + IP header to get addresses
        let (src_addr, dst_addr) = self.parse_packet_addresses(data)?;
        
        let registry = self.session_registry.read();
        
        // First try direct address lookup
        if let Some(session_id) = registry.lookup_by_either(&src_addr, &dst_addr) {
            return Some(session_id.clone());
        }
        
        // For ICMP packets, try to extract the embedded original packet destination
        if self.is_icmp_error_packet(data) {
            if let Some(embedded_dst) = self.extract_embedded_destination(data) {
                // Look up by the embedded destination port (which is our server port)
                if let Some(session_id) = registry.lookup_by_server_port(embedded_dst.port()) {
                    return Some(session_id.clone());
                }
                // Also try direct lookup of embedded destination
                if let Some(session_id) = registry.lookup(&embedded_dst) {
                    return Some(session_id.clone());
                }
            }
        }
        
        None
    }
    
    /// Parse packet addresses (source and destination) from raw packet data
    /// Returns (src_addr, dst_addr) if the packet is UDP
    fn parse_packet_addresses(&self, data: &[u8]) -> Option<(SocketAddr, SocketAddr)> {
        // Minimum: Ethernet (14) + IP header (20) + UDP header (8) = 42 bytes
        if data.len() < 42 {
            return None;
        }
        
        // Check EtherType - we support both raw IP and Ethernet frames
        let ip_start = if data.len() >= 14 {
            let ethertype = u16::from_be_bytes([data[12], data[13]]);
            match ethertype {
                0x0800 => 14,  // IPv4
                0x86DD => 14,  // IPv6
                _ if (data[0] >> 4) == 4 || (data[0] >> 4) == 6 => 0, // Raw IP (Linux cooked capture)
                _ => return None,
            }
        } else {
            0 // Assume raw IP for short packets
        };
        
        if ip_start >= data.len() {
            return None;
        }
        
        let ip_version = (data[ip_start] >> 4) & 0x0F;
        
        match ip_version {
            4 => self.parse_ipv4_udp_addresses(data, ip_start),
            6 => self.parse_ipv6_udp_addresses(data, ip_start),
            _ => None,
        }
    }
    
    /// Parse IPv4 UDP addresses
    fn parse_ipv4_udp_addresses(&self, data: &[u8], ip_start: usize) -> Option<(SocketAddr, SocketAddr)> {
        if data.len() < ip_start + 20 {
            return None;
        }
        
        let ihl = ((data[ip_start] & 0x0F) * 4) as usize;
        let protocol = data[ip_start + 9];
        
        // Only process UDP (17) and ICMP (1)
        if protocol != 17 && protocol != 1 {
            return None;
        }
        
        let src_ip = IpAddr::V4(std::net::Ipv4Addr::new(
            data[ip_start + 12],
            data[ip_start + 13],
            data[ip_start + 14],
            data[ip_start + 15],
        ));
        
        let dst_ip = IpAddr::V4(std::net::Ipv4Addr::new(
            data[ip_start + 16],
            data[ip_start + 17],
            data[ip_start + 18],
            data[ip_start + 19],
        ));
        
        let udp_start = ip_start + ihl;
        if data.len() < udp_start + 8 {
            // For ICMP, use port 0
            if protocol == 1 {
                return Some((
                    SocketAddr::new(src_ip, 0),
                    SocketAddr::new(dst_ip, 0),
                ));
            }
            return None;
        }
        
        // For ICMP, use port 0
        if protocol == 1 {
            return Some((
                SocketAddr::new(src_ip, 0),
                SocketAddr::new(dst_ip, 0),
            ));
        }
        
        let src_port = u16::from_be_bytes([data[udp_start], data[udp_start + 1]]);
        let dst_port = u16::from_be_bytes([data[udp_start + 2], data[udp_start + 3]]);
        
        Some((
            SocketAddr::new(src_ip, src_port),
            SocketAddr::new(dst_ip, dst_port),
        ))
    }
    
    /// Parse IPv6 UDP addresses
    fn parse_ipv6_udp_addresses(&self, data: &[u8], ip_start: usize) -> Option<(SocketAddr, SocketAddr)> {
        if data.len() < ip_start + 40 {
            return None;
        }
        
        let next_header = data[ip_start + 6];
        
        // Only process UDP (17) and ICMPv6 (58)
        if next_header != 17 && next_header != 58 {
            return None;
        }
        
        let src_bytes: [u8; 16] = data[ip_start + 8..ip_start + 24].try_into().ok()?;
        let dst_bytes: [u8; 16] = data[ip_start + 24..ip_start + 40].try_into().ok()?;
        
        let src_ip = IpAddr::V6(std::net::Ipv6Addr::from(src_bytes));
        let dst_ip = IpAddr::V6(std::net::Ipv6Addr::from(dst_bytes));
        
        let udp_start = ip_start + 40;
        
        // For ICMPv6, use port 0
        if next_header == 58 {
            return Some((
                SocketAddr::new(src_ip, 0),
                SocketAddr::new(dst_ip, 0),
            ));
        }
        
        if data.len() < udp_start + 8 {
            return None;
        }
        
        let src_port = u16::from_be_bytes([data[udp_start], data[udp_start + 1]]);
        let dst_port = u16::from_be_bytes([data[udp_start + 2], data[udp_start + 3]]);
        
        Some((
            SocketAddr::new(src_ip, src_port),
            SocketAddr::new(dst_ip, dst_port),
        ))
    }
    
    /// Check if packet is an ICMP error packet
    fn is_icmp_error_packet(&self, data: &[u8]) -> bool {
        // Minimum size check
        if data.len() < 34 {  // Ethernet + IP + ICMP type
            return false;
        }
        
        // Determine IP start
        let ip_start = if data.len() >= 14 {
            let ethertype = u16::from_be_bytes([data[12], data[13]]);
            match ethertype {
                0x0800 | 0x86DD => 14,
                _ if (data[0] >> 4) == 4 || (data[0] >> 4) == 6 => 0,
                _ => return false,
            }
        } else {
            0
        };
        
        if ip_start >= data.len() {
            return false;
        }
        
        let ip_version = (data[ip_start] >> 4) & 0x0F;
        
        match ip_version {
            4 => {
                if data.len() < ip_start + 21 {
                    return false;
                }
                let protocol = data[ip_start + 9];
                if protocol != 1 {  // ICMP
                    return false;
                }
                let ihl = ((data[ip_start] & 0x0F) * 4) as usize;
                if data.len() < ip_start + ihl + 1 {
                    return false;
                }
                let icmp_type = data[ip_start + ihl];
                // ICMP error types: 3 (Dest Unreachable), 11 (Time Exceeded), 12 (Parameter Problem)
                matches!(icmp_type, 3 | 11 | 12)
            }
            6 => {
                if data.len() < ip_start + 41 {
                    return false;
                }
                let next_header = data[ip_start + 6];
                if next_header != 58 {  // ICMPv6
                    return false;
                }
                let icmpv6_type = data[ip_start + 40];
                // ICMPv6 error types: 1 (Dest Unreachable), 2 (Packet Too Big), 3 (Time Exceeded)
                matches!(icmpv6_type, 1 | 2 | 3)
            }
            _ => false,
        }
    }
    
    /// Extract the embedded original packet destination from an ICMP error
    fn extract_embedded_destination(&self, data: &[u8]) -> Option<SocketAddr> {
        // Determine IP start
        let ip_start = if data.len() >= 14 {
            let ethertype = u16::from_be_bytes([data[12], data[13]]);
            match ethertype {
                0x0800 | 0x86DD => 14,
                _ if (data[0] >> 4) == 4 || (data[0] >> 4) == 6 => 0,
                _ => return None,
            }
        } else {
            0
        };
        
        let ip_version = (data[ip_start] >> 4) & 0x0F;
        
        match ip_version {
            4 => self.extract_embedded_ipv4_destination(data, ip_start),
            6 => self.extract_embedded_ipv6_destination(data, ip_start),
            _ => None,
        }
    }
    
    /// Extract embedded IPv4 destination from ICMP error
    fn extract_embedded_ipv4_destination(&self, data: &[u8], ip_start: usize) -> Option<SocketAddr> {
        let ihl = ((data[ip_start] & 0x0F) * 4) as usize;
        let icmp_start = ip_start + ihl;
        
        // ICMP header is 8 bytes, then embedded IP packet
        let embedded_ip_start = icmp_start + 8;
        
        if data.len() < embedded_ip_start + 28 {  // Embedded IP (20) + UDP header (8)
            return None;
        }
        
        let embedded_ihl = ((data[embedded_ip_start] & 0x0F) * 4) as usize;
        let embedded_protocol = data[embedded_ip_start + 9];
        
        // Only handle UDP
        if embedded_protocol != 17 {
            return None;
        }
        
        let dst_ip = std::net::Ipv4Addr::new(
            data[embedded_ip_start + 16],
            data[embedded_ip_start + 17],
            data[embedded_ip_start + 18],
            data[embedded_ip_start + 19],
        );
        
        let embedded_udp_start = embedded_ip_start + embedded_ihl;
        if data.len() < embedded_udp_start + 4 {
            return None;
        }
        
        let dst_port = u16::from_be_bytes([data[embedded_udp_start + 2], data[embedded_udp_start + 3]]);
        
        Some(SocketAddr::new(IpAddr::V4(dst_ip), dst_port))
    }
    
    /// Extract embedded IPv6 destination from ICMPv6 error
    fn extract_embedded_ipv6_destination(&self, data: &[u8], ip_start: usize) -> Option<SocketAddr> {
        let icmpv6_start = ip_start + 40;  // IPv6 header is fixed 40 bytes
        
        // ICMPv6 header is 8 bytes, then embedded IPv6 packet
        let embedded_ip_start = icmpv6_start + 8;
        
        if data.len() < embedded_ip_start + 48 {  // Embedded IPv6 (40) + UDP header (8)
            return None;
        }
        
        let embedded_next_header = data[embedded_ip_start + 6];
        
        // Only handle UDP
        if embedded_next_header != 17 {
            return None;
        }
        
        let dst_bytes: [u8; 16] = data[embedded_ip_start + 24..embedded_ip_start + 40].try_into().ok()?;
        let dst_ip = std::net::Ipv6Addr::from(dst_bytes);
        
        let embedded_udp_start = embedded_ip_start + 40;
        if data.len() < embedded_udp_start + 4 {
            return None;
        }
        
        let dst_port = u16::from_be_bytes([data[embedded_udp_start + 2], data[embedded_udp_start + 3]]);
        
        Some(SocketAddr::new(IpAddr::V6(dst_ip), dst_port))
    }

    /// Get all captured packets in chronological order
    #[allow(dead_code)]
    pub fn get_packets(&self) -> Vec<CapturedPacket> {
        self.buffer.read().get_packets()
    }
    
    /// Get packets for a specific survey session
    pub fn get_packets_for_session(&self, survey_session_id: &str) -> Vec<CapturedPacket> {
        self.buffer.read().get_packets_for_session(survey_session_id)
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

        Self::packets_to_pcap(&packets, datalink, snaplen)
    }
    
    /// Generate PCAP file contents for a specific survey session
    pub fn generate_pcap_for_session(&self, survey_session_id: &str) -> Vec<u8> {
        let buffer = self.buffer.read();
        let packets = buffer.get_packets_for_session(survey_session_id);
        let datalink = buffer.datalink();
        let snaplen = self.config.snaplen as u32;
        drop(buffer);

        Self::packets_to_pcap(&packets, datalink, snaplen)
    }
    
    /// Convert packets to PCAP format
    fn packets_to_pcap(packets: &[CapturedPacket], datalink: i32, snaplen: u32) -> Vec<u8> {
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

                service.add_packet(ts_sec, ts_usec.into(), orig_len, data);
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
        buffer.add_packet(1000, 0, 3, vec![1, 2, 3], None);
        buffer.add_packet(1001, 0, 3, vec![4, 5, 6], None);
        buffer.add_packet(1002, 0, 3, vec![7, 8, 9], None);

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
        buffer.add_packet(1000, 0, 1, vec![1], None);
        buffer.add_packet(1001, 0, 1, vec![2], None);
        buffer.add_packet(1002, 0, 1, vec![3], None);
        buffer.add_packet(1003, 0, 1, vec![4], None);
        buffer.add_packet(1004, 0, 1, vec![5], None);

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

        // Add a test packet (packet too short to extract session)
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
    
    #[test]
    fn test_session_registry() {
        let mut registry = SessionRegistry::new();
        
        let addr1 = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 100)), 54321);
        let addr2 = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 101)), 54322);
        
        registry.register(addr1, 443, "session-a".to_string());
        registry.register(addr2, 443, "session-b".to_string());
        
        assert_eq!(registry.lookup(&addr1), Some(&"session-a".to_string()));
        assert_eq!(registry.lookup(&addr2), Some(&"session-b".to_string()));
        
        // Look up by server port
        assert!(registry.lookup_by_server_port(443).is_some());
        
        registry.unregister(&addr1);
        assert_eq!(registry.lookup(&addr1), None);
    }
    
    #[test]
    fn test_packets_for_session() {
        let config = CaptureConfig {
            max_packets: 10,
            snaplen: 100,
            ..Default::default()
        };
        let mut buffer = PacketRingBuffer::new(config);
        
        // Add packets with different session IDs
        buffer.add_packet(1000, 0, 3, vec![1, 2, 3], Some("session-a".to_string()));
        buffer.add_packet(1001, 0, 3, vec![4, 5, 6], Some("session-b".to_string()));
        buffer.add_packet(1002, 0, 3, vec![7, 8, 9], Some("session-a".to_string()));
        buffer.add_packet(1003, 0, 3, vec![10, 11, 12], None);
        
        let session_a_packets = buffer.get_packets_for_session("session-a");
        assert_eq!(session_a_packets.len(), 2);
        assert_eq!(session_a_packets[0].data, vec![1, 2, 3]);
        assert_eq!(session_a_packets[1].data, vec![7, 8, 9]);
        
        let session_b_packets = buffer.get_packets_for_session("session-b");
        assert_eq!(session_b_packets.len(), 1);
        assert_eq!(session_b_packets[0].data, vec![4, 5, 6]);
        
        // All packets
        let all_packets = buffer.get_packets();
        assert_eq!(all_packets.len(), 4);
    }
}
