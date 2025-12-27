use std::sync::Arc;
use tokio::time::{interval, Duration};
use common::{ProbePacket, Direction, BulkPacket, ClientMetrics};
use crate::state::{ClientSession, ReceivedProbe, ReceivedBulk, SentBulk};
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use webrtc::data_channel::data_channel_message::DataChannelMessage;

pub async fn start_probe_sender(
    session: Arc<ClientSession>,
) {
    let mut interval = interval(Duration::from_millis(50)); // 20 Hz

    loop {
        interval.tick().await;

        // Check if probe channel is ready
        let channels = session.data_channels.read().await;
        let probe_channel = match &channels.probe {
            Some(ch) if ch.ready_state() == RTCDataChannelState::Open => ch.clone(),
            _ => {
                drop(channels);
                continue;
            }
        };
        drop(channels);

        // Create and send probe packet
        let sent_at_ms = current_time_ms();
        let mut state = session.measurement_state.write().await;
        let seq = state.probe_seq;
        state.probe_seq += 1;

        // Track sent probe for S2C delay calculation
        state.sent_probes.push_back(crate::state::SentProbe {
            seq,
            sent_at_ms,
        });

        // Keep only last 60 seconds of sent probes
        let cutoff = sent_at_ms - 60_000;
        while let Some(p) = state.sent_probes.front() {
            if p.sent_at_ms < cutoff {
                state.sent_probes.pop_front();
            } else {
                break;
            }
        }

        drop(state);

        let probe = ProbePacket {
            seq,
            timestamp_ms: sent_at_ms,
            direction: Direction::ServerToClient,
            send_options: None,  // Will be enhanced later to support per-packet options
        };

        if let Ok(json) = serde_json::to_vec(&probe) {
            if let Err(e) = probe_channel.send(&json.into()).await {
                tracing::error!("Failed to send probe: {}", e);
                break;
            }
        }
    }
}

pub async fn start_bulk_sender(
    session: Arc<ClientSession>,
) {
    let mut interval = interval(Duration::from_millis(10)); // 100 Hz for continuous throughput

    loop {
        interval.tick().await;

        let channels = session.data_channels.read().await;
        let bulk_channel = match &channels.bulk {
            Some(ch) if ch.ready_state() == RTCDataChannelState::Open => ch.clone(),
            _ => {
                drop(channels);
                continue;
            }
        };
        drop(channels);

        let bulk = BulkPacket::new(1024);

        if let Ok(data) = serde_json::to_vec(&bulk) {
            let bytes_sent = data.len() as u64;
            let sent_at_ms = current_time_ms();

            if let Err(e) = bulk_channel.send(&data.into()).await {
                tracing::error!("Failed to send bulk: {}", e);
                break;
            }

            let mut state = session.measurement_state.write().await;
            state.bulk_bytes_sent += bytes_sent;
            state.sent_bulk_packets.push_back(SentBulk {
                bytes: bytes_sent,
                sent_at_ms,
            });

            // Keep only last 60 seconds of sent bulk packets
            let cutoff = sent_at_ms - 60_000;
            while let Some(b) = state.sent_bulk_packets.front() {
                if b.sent_at_ms < cutoff {
                    state.sent_bulk_packets.pop_front();
                } else {
                    break;
                }
            }
        }
    }
}

pub async fn handle_probe_packet(
    session: Arc<ClientSession>,
    msg: DataChannelMessage,
) {
    if let Ok(probe) = serde_json::from_slice::<ProbePacket>(&msg.data) {
        let now_ms = current_time_ms();

        let mut state = session.measurement_state.write().await;

        // Check if this is an echoed S2C probe or a C2S probe
        if probe.direction == Direction::ServerToClient {
            // This is an echoed probe - client received our probe and echoed it back
            // probe.timestamp_ms is when client received it (client's echo timestamp)
            // We need to find the original sent probe to get the original sent time
            tracing::debug!("Received echoed S2C probe seq {} from client {}", probe.seq, session.id);

            if let Some(sent_probe) = state.sent_probes.iter().find(|p| p.seq == probe.seq) {
                let sent_at_ms = sent_probe.sent_at_ms;  // Clone to avoid borrow issue
                state.echoed_probes.push_back(crate::state::EchoedProbe {
                    seq: probe.seq,
                    sent_at_ms,
                    echoed_at_ms: probe.timestamp_ms,
                });
                tracing::debug!("Matched echoed probe seq {}, delay: {}ms",
                    probe.seq, probe.timestamp_ms.saturating_sub(sent_at_ms));

                // Keep only last 60 seconds of echoed probes
                let cutoff = now_ms - 60_000;
                while let Some(p) = state.echoed_probes.front() {
                    if p.echoed_at_ms < cutoff {
                        state.echoed_probes.pop_front();
                    } else {
                        break;
                    }
                }
            } else {
                tracing::warn!("Received echoed probe seq {} but couldn't find matching sent probe", probe.seq);
            }
        } else {
            // This is a C2S probe from client
            state.received_probes.push_back(ReceivedProbe {
                seq: probe.seq,
                sent_at_ms: probe.timestamp_ms,
                received_at_ms: now_ms,
            });

            // Keep only last 60 seconds of probes
            let cutoff = now_ms - 60_000;
            while let Some(p) = state.received_probes.front() {
                if p.received_at_ms < cutoff {
                    state.received_probes.pop_front();
                } else {
                    break;
                }
            }
        }

        drop(state);

        // Recalculate metrics
        calculate_metrics(session).await;
    }
}

pub async fn handle_bulk_packet(
    session: Arc<ClientSession>,
    msg: DataChannelMessage,
) {
    let now_ms = current_time_ms();
    let bytes = msg.data.len() as u64;

    let mut state = session.measurement_state.write().await;
    state.received_bulk_bytes.push_back(ReceivedBulk {
        bytes,
        received_at_ms: now_ms,
    });

    // Keep only last 60 seconds
    let cutoff = now_ms - 60_000;
    while let Some(b) = state.received_bulk_bytes.front() {
        if b.received_at_ms < cutoff {
            state.received_bulk_bytes.pop_front();
        } else {
            break;
        }
    }

    drop(state);
    calculate_metrics(session).await;
}

async fn calculate_metrics(session: Arc<ClientSession>) {
    let state = session.measurement_state.read().await;
    let now_ms = current_time_ms();

    let mut metrics = ClientMetrics::default();

    // Calculate for each time window: 1s, 10s, 60s
    let windows = [1_000u64, 10_000, 60_000];

    for (i, &window_ms) in windows.iter().enumerate() {
        let cutoff = now_ms.saturating_sub(window_ms);

        // Client-to-server metrics (from received probes)
        let recent_probes: Vec<_> = state.received_probes.iter()
            .filter(|p| p.received_at_ms >= cutoff)
            .collect();

        if !recent_probes.is_empty() {
            // Calculate delay (use saturating_sub to prevent overflow if client clock is ahead)
            let delays: Vec<f64> = recent_probes.iter()
                .map(|p| (p.received_at_ms.saturating_sub(p.sent_at_ms)) as f64)
                .collect();

            let avg_delay = delays.iter().sum::<f64>() / delays.len() as f64;
            metrics.c2s_delay_avg[i] = avg_delay;

            // Calculate jitter (std dev of delay)
            let variance = delays.iter()
                .map(|d| (d - avg_delay).powi(2))
                .sum::<f64>() / delays.len() as f64;
            metrics.c2s_jitter[i] = variance.sqrt();

            // Calculate loss rate
            if recent_probes.len() >= 2 {
                let min_seq = recent_probes.iter().map(|p| p.seq).min().unwrap();
                let max_seq = recent_probes.iter().map(|p| p.seq).max().unwrap();
                let expected = (max_seq - min_seq + 1) as f64;
                let received = recent_probes.len() as f64;
                metrics.c2s_loss_rate[i] = ((expected - received) / expected * 100.0).max(0.0);
            }

            // Calculate reordering rate
            // Track max sequence seen so far; any packet with seq < max is reordered
            let mut reorders = 0;
            let mut max_seq_seen = 0u64;
            for p in &recent_probes {
                if p.seq < max_seq_seen {
                    reorders += 1;
                }
                max_seq_seen = max_seq_seen.max(p.seq);
            }
            metrics.c2s_reorder_rate[i] = (reorders as f64 / recent_probes.len() as f64) * 100.0;
        }

        // Client-to-server throughput (from received bulk)
        let recent_bulk: Vec<_> = state.received_bulk_bytes.iter()
            .filter(|b| b.received_at_ms >= cutoff)
            .collect();

        if !recent_bulk.is_empty() {
            let total_bytes: u64 = recent_bulk.iter().map(|b| b.bytes).sum();
            let time_window_sec = window_ms as f64 / 1000.0;
            metrics.c2s_throughput[i] = total_bytes as f64 / time_window_sec;
        }

        // Server-to-client throughput (from sent bulk)
        let recent_sent_bulk: Vec<_> = state.sent_bulk_packets.iter()
            .filter(|b| b.sent_at_ms >= cutoff)
            .collect();

        if !recent_sent_bulk.is_empty() {
            let total_bytes: u64 = recent_sent_bulk.iter().map(|b| b.bytes).sum();
            let time_window_sec = window_ms as f64 / 1000.0;
            metrics.s2c_throughput[i] = total_bytes as f64 / time_window_sec;
        }

        // Server-to-client metrics (from echoed probes)
        let recent_echoed_probes: Vec<_> = state.echoed_probes.iter()
            .filter(|p| p.echoed_at_ms >= cutoff)
            .collect();

        if !recent_echoed_probes.is_empty() {
            // Calculate delay (time from sent to echoed)
            let delays: Vec<f64> = recent_echoed_probes.iter()
                .map(|p| (p.echoed_at_ms.saturating_sub(p.sent_at_ms)) as f64)
                .collect();

            let avg_delay = delays.iter().sum::<f64>() / delays.len() as f64;
            metrics.s2c_delay_avg[i] = avg_delay;

            // Calculate jitter (std dev of delay)
            let variance = delays.iter()
                .map(|d| (d - avg_delay).powi(2))
                .sum::<f64>() / delays.len() as f64;
            metrics.s2c_jitter[i] = variance.sqrt();

            // Calculate loss rate
            if recent_echoed_probes.len() >= 2 {
                let min_seq = recent_echoed_probes.iter().map(|p| p.seq).min().unwrap();
                let max_seq = recent_echoed_probes.iter().map(|p| p.seq).max().unwrap();
                let expected = (max_seq - min_seq + 1) as f64;
                let received = recent_echoed_probes.len() as f64;
                metrics.s2c_loss_rate[i] = ((expected - received) / expected * 100.0).max(0.0);
            }

            // Calculate reordering rate
            let mut reorders = 0;
            let mut max_seq_seen = 0u64;
            for p in &recent_echoed_probes {
                if p.seq < max_seq_seen {
                    reorders += 1;
                }
                max_seq_seen = max_seq_seen.max(p.seq);
            }
            metrics.s2c_reorder_rate[i] = (reorders as f64 / recent_echoed_probes.len() as f64) * 100.0;
        }
    }

    drop(state);

    // Update session metrics
    *session.metrics.write().await = metrics;
}

fn format_traceroute_message(hop: u8, router_ip: &Option<String>, rtt_ms: f64) -> String {
    if let Some(ip) = router_ip {
        format!("Hop {} via {} ({:.2}ms)", hop, ip, rtt_ms)
    } else {
        format!("Hop {} received ({:.2}ms)", hop, rtt_ms)
    }
}

pub async fn start_traceroute_sender(
    session: Arc<ClientSession>,
) {
    tracing::info!("Starting traceroute sender for session {}", session.id);
    
    let mut interval = interval(Duration::from_secs(1)); // Send one hop discovery per second
    let mut current_ttl: u8 = 1;
    const MAX_TTL: u8 = 30;

    loop {
        interval.tick().await;
        tracing::debug!("Traceroute tick for session {}, TTL {}", session.id, current_ttl);

        // Check if control channel is ready
        let channels = session.data_channels.read().await;
        let control_channel = match &channels.control {
            Some(ch) if ch.ready_state() == RTCDataChannelState::Open => ch.clone(),
            _ => {
                tracing::debug!("Control channel not ready for session {}, skipping", session.id);
                drop(channels);
                continue;
            }
        };
        drop(channels);

        // Get probe channel to send traceroute probes
        let channels = session.data_channels.read().await;
        let probe_channel = match &channels.probe {
            Some(ch) if ch.ready_state() == RTCDataChannelState::Open => ch.clone(),
            _ => {
                tracing::debug!("Probe channel not ready for session {}, skipping", session.id);
                drop(channels);
                continue;
            }
        };
        drop(channels);

        // Create probe packet with specific TTL for traceroute
        let sent_at_ms = current_time_ms();
        let seq = {
            let mut state = session.measurement_state.write().await;
            let seq = state.probe_seq;
            state.probe_seq += 1;
            
            // Track the probe so we can match it when/if it's echoed back
            state.sent_probes.push_back(crate::state::SentProbe {
                seq,
                sent_at_ms,
            });
            
            // Keep only last 60 seconds of sent probes
            let cutoff = sent_at_ms - 60_000;
            while let Some(p) = state.sent_probes.front() {
                if p.sent_at_ms < cutoff {
                    state.sent_probes.pop_front();
                } else {
                    break;
                }
            }
            
            seq
        };

        let send_options = common::SendOptions {
            ttl: Some(current_ttl),
            df_bit: Some(true),
            tos: None,
            flow_label: None,
            track_for_ms: 5000, // Track for 5 seconds to catch ICMP responses
        };

        let probe = ProbePacket {
            seq,
            timestamp_ms: sent_at_ms,
            direction: Direction::ServerToClient,
            send_options: Some(send_options),
        };

        // Send probe with TTL using the new send_with_options API
        if let Ok(mut json) = serde_json::to_vec(&probe) {
            // Pad the JSON to create unique lengths for each TTL
            // This helps with matching ICMP errors based on UDP packet length
            // Base size + (TTL * 10 bytes) to make each hop distinguishable
            let target_size = 100 + (current_ttl as usize * 10);
            if json.len() < target_size {
                json.resize(target_size, b' '); // Pad with spaces
            }
            
            tracing::info!("🔵 Sending traceroute probe via data channel: TTL={}, seq={}, json_len={}", 
                current_ttl, seq, json.len());
            
            #[cfg(target_os = "linux")]
            let send_result = {
                use webrtc_util::UdpSendOptions;
                let options = Some(UdpSendOptions {
                    ttl: Some(current_ttl),
                    tos: None,
                    df_bit: Some(true),
                });
                tracing::info!("🔵 Created UdpSendOptions: TTL={:?}, TOS={:?}, DF={:?}", 
                    options.as_ref().and_then(|o| o.ttl),
                    options.as_ref().and_then(|o| o.tos),
                    options.as_ref().and_then(|o| o.df_bit));
                probe_channel.send_with_options(&json.into(), options).await
            };
            
            #[cfg(not(target_os = "linux"))]
            let send_result = probe_channel.send(&json.into()).await;

            if let Err(e) = send_result {
                tracing::error!("Failed to send traceroute probe: {}", e);
                continue;
            }

            tracing::info!("Sent traceroute probe with TTL {} (seq {})", current_ttl, seq);
            
            // Note: Packet tracking now happens automatically at the UDP layer
            // The vendored webrtc-util code will call wifi_verify_track_udp_packet()
            // with exact measurements when the packet is actually sent via sendmsg

            // Wait a bit for potential ICMP response
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Check for ICMP events from the packet tracker
            let events = session.packet_tracker.drain_events().await;
            
            if !events.is_empty() {
                tracing::info!("Processing {} ICMP events for traceroute", events.len());
                
                for event in events {
                    // Extract TTL from send options to determine hop number
                    // TTL should always be set in send_options for traceroute packets
                    let hop = event.send_options.ttl.expect("TTL should be set for traceroute packets");
                    
                    // Calculate RTT in milliseconds
                    let rtt = event.icmp_received_at.duration_since(event.sent_at);
                    let rtt_ms = rtt.as_secs_f64() * 1000.0;
                    
                    // Create hop message with actual ICMP data
                    let hop_message = common::TraceHopMessage {
                        hop,
                        ip_address: event.router_ip.clone(),
                        rtt_ms,
                        message: format_traceroute_message(hop, &event.router_ip, rtt_ms),
                    };

                    tracing::info!("✅ Sending traceroute hop message: hop={}, ip={:?}, rtt={:.2}ms", 
                        hop, event.router_ip, rtt_ms);

                    if let Ok(msg_json) = serde_json::to_vec(&hop_message) {
                        if let Err(e) = control_channel.send(&msg_json.into()).await {
                            tracing::error!("Failed to send hop message to client: {}", e);
                        }
                    }
                }
            } else {
                // No ICMP response received yet - send placeholder message
                let hop_message = common::TraceHopMessage {
                    hop: current_ttl,
                    ip_address: None,
                    rtt_ms: 0.0,
                    message: format!("Probing hop {} (seq: {})", current_ttl, seq),
                };

                if let Ok(msg_json) = serde_json::to_vec(&hop_message) {
                    if let Err(e) = control_channel.send(&msg_json.into()).await {
                        tracing::error!("Failed to send hop message to client: {}", e);
                    }
                }
            }
        }

        // Increment TTL for next probe
        current_ttl += 1;
        if current_ttl > MAX_TTL {
            current_ttl = 1; // Reset to start over
        }
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_time_ms() {
        let t1 = current_time_ms();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = current_time_ms();
        assert!(t2 > t1);
        assert!(t2 - t1 >= 10);
    }
}
