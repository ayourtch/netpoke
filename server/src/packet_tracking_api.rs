/// API handlers for packet tracking and ICMP events
use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use crate::state::AppState;
use base64::{Engine as _, engine::general_purpose};

/// Response structure for tracked packet events
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackedEventsResponse {
    pub events: Vec<TrackedEventInfo>,
    pub count: usize,
}

/// Serializable version of TrackedPacketEvent
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackedEventInfo {
    pub icmp_packet_size: usize,
    pub udp_packet_size: usize,
    pub cleartext_size: usize,
    pub sent_at_ms: u64,
    pub icmp_received_at_ms: u64,
    pub rtt_ms: u64,
    pub send_options: common::SendOptions,
    pub router_ip: Option<String>,
    
    /// Base64 encoded packets for inspection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icmp_packet_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_packet_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleartext_b64: Option<String>,
}

/// Get all tracked packet events and clear the queue
pub async fn get_tracked_events(
    State(state): State<AppState>,
) -> Result<Json<TrackedEventsResponse>, StatusCode> {
    let events = state.packet_tracker.drain_events().await;
    
    let event_infos: Vec<TrackedEventInfo> = events
        .into_iter()
        .map(|event| {
            let sent_duration = event.sent_at.elapsed();
            let icmp_duration = event.icmp_received_at.elapsed();
            let rtt = event.icmp_received_at.duration_since(event.sent_at);
            
            TrackedEventInfo {
                icmp_packet_size: event.icmp_packet.len(),
                udp_packet_size: event.udp_packet.len(),
                cleartext_size: event.cleartext.len(),
                sent_at_ms: sent_duration.as_millis() as u64,
                icmp_received_at_ms: icmp_duration.as_millis() as u64,
                rtt_ms: rtt.as_millis() as u64,
                send_options: event.send_options,
                router_ip: event.router_ip,
                icmp_packet_b64: Some(general_purpose::STANDARD.encode(&event.icmp_packet)),
                udp_packet_b64: Some(general_purpose::STANDARD.encode(&event.udp_packet)),
                cleartext_b64: Some(general_purpose::STANDARD.encode(&event.cleartext)),
            }
        })
        .collect();
    
    let count = event_infos.len();
    
    Ok(Json(TrackedEventsResponse {
        events: event_infos,
        count,
    }))
}

/// Get tracked packet statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackedStatsResponse {
    pub tracked_packets_count: usize,
    pub queued_events_count: usize,
}

pub async fn get_tracked_stats(
    State(state): State<AppState>,
) -> Result<Json<TrackedStatsResponse>, StatusCode> {
    let tracked_count = state.packet_tracker.tracked_count().await;
    let events = state.packet_tracker.drain_events().await;
    let queued_count = events.len();
    
    // Put events back
    for event in events {
        state.packet_tracker.event_queue.write().await.push(event);
    }
    
    Ok(Json(TrackedStatsResponse {
        tracked_packets_count: tracked_count,
        queued_events_count: queued_count,
    }))
}
