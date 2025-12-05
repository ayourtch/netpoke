use serde::{Deserialize, Serialize};
use crate::metrics::ClientMetrics;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Direction {
    ClientToServer,
    ServerToClient,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbePacket {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub direction: Direction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkPacket {
    pub data: Vec<u8>,
}

impl BulkPacket {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: String,
    pub connected_at: u64, // Unix timestamp
    pub metrics: ClientMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMessage {
    pub clients: Vec<ClientInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_packet_serialization() {
        let packet = ProbePacket {
            seq: 42,
            timestamp_ms: 1234567890,
            direction: Direction::ClientToServer,
        };

        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: ProbePacket = serde_json::from_str(&json).unwrap();

        assert_eq!(packet, deserialized);
    }

    #[test]
    fn test_bulk_packet_creation() {
        let packet = BulkPacket::new(1024);
        assert_eq!(packet.data.len(), 1024);
    }

    #[test]
    fn test_dashboard_message_serialization() {
        let msg = DashboardMessage {
            clients: vec![
                ClientInfo {
                    id: "client-1".to_string(),
                    connected_at: 1234567890,
                    metrics: ClientMetrics::default(),
                }
            ],
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: DashboardMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(msg.clients.len(), deserialized.clients.len());
    }
}
