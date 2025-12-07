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
        let mut state = session.measurement_state.write().await;
        let seq = state.probe_seq;
        state.probe_seq += 1;
        drop(state);

        let probe = ProbePacket {
            seq,
            timestamp_ms: current_time_ms(),
            direction: Direction::ServerToClient,
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
    }

    drop(state);

    // Update session metrics
    *session.metrics.write().await = metrics;
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
