use crate::state::{ClientSession, ReceivedBulk, ReceivedProbe, SentBulk};
use common::{BulkPacket, ClientMetrics, Direction, ProbePacket};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use webrtc::data_channel::RTCDataChannel;

pub async fn start_probe_sender(session: Arc<ClientSession>) {
    let mut interval = interval(Duration::from_millis(50)); // 20 Hz

    loop {
        interval.tick().await;

        // Check if traffic should still be active
        {
            let state = session.measurement_state.read().await;
            if !state.traffic_active {
                tracing::info!(
                    "Stopping probe sender for session {} (traffic_active=false)",
                    session.id
                );
                break;
            }
        }

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
        let sent_probe = crate::state::SentProbe { seq, sent_at_ms };
        state.sent_probes.push_back(sent_probe.clone());
        state.sent_probes_map.insert(seq, sent_probe);

        // Keep only last 60 seconds of sent probes
        let cutoff = sent_at_ms - 60_000;
        while let Some(p) = state.sent_probes.front() {
            if p.sent_at_ms < cutoff {
                let old_probe = state.sent_probes.pop_front().unwrap();
                state.sent_probes_map.remove(&old_probe.seq);
            } else {
                break;
            }
        }

        drop(state);

        let probe = ProbePacket {
            seq,
            timestamp_ms: sent_at_ms,
            direction: Direction::ServerToClient,
            send_options: None, // Will be enhanced later to support per-packet options
            conn_id: session.conn_id.clone(),
        };

        if let Ok(json) = serde_json::to_vec(&probe) {
            if let Err(e) = probe_channel.send(&json.into()).await {
                tracing::error!("Failed to send probe: {}", e);
                break;
            }
        }
    }
}

pub async fn start_bulk_sender(session: Arc<ClientSession>) {
    let mut interval = interval(Duration::from_millis(10)); // 100 Hz for continuous throughput

    loop {
        interval.tick().await;

        // Check if traffic should still be active
        {
            let state = session.measurement_state.read().await;
            if !state.traffic_active {
                tracing::info!(
                    "Stopping bulk sender for session {} (traffic_active=false)",
                    session.id
                );
                break;
            }
        }

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

pub async fn handle_probe_packet(session: Arc<ClientSession>, msg: DataChannelMessage) {
    if let Ok(probe) = serde_json::from_slice::<ProbePacket>(&msg.data) {
        // Validate conn_id - ensure probe belongs to this session
        if probe.conn_id != session.conn_id {
            tracing::warn!(
                "Probe conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                probe.conn_id,
                session.id,
                session.conn_id
            );
            return;
        }

        let now_ms = current_time_ms();

        let mut state = session.measurement_state.write().await;

        // Check if this is an echoed S2C probe or a C2S probe
        if probe.direction == Direction::ServerToClient {
            // This is an echoed probe - client received our probe and echoed it back
            // probe.timestamp_ms is when client received it (client's echo timestamp)
            // We need to find the original sent probe to get the original sent time
            tracing::debug!(
                "Received echoed S2C probe seq {} from client {}",
                probe.seq,
                session.id
            );

            // Use HashMap for O(1) lookup instead of linear search
            if let Some(sent_probe) = state.sent_probes_map.get(&probe.seq) {
                let sent_at_ms = sent_probe.sent_at_ms;
                state.echoed_probes.push_back(crate::state::EchoedProbe {
                    seq: probe.seq,
                    sent_at_ms,
                    echoed_at_ms: probe.timestamp_ms,
                });
                tracing::debug!(
                    "Matched echoed probe seq {}, delay: {}ms",
                    probe.seq,
                    probe.timestamp_ms as i64 - sent_at_ms as i64
                );

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
                tracing::warn!(
                    "Received echoed probe seq {} but couldn't find matching sent probe",
                    probe.seq
                );
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

pub async fn handle_bulk_packet(session: Arc<ClientSession>, msg: DataChannelMessage) {
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
        let recent_probes: Vec<_> = state
            .received_probes
            .iter()
            .filter(|p| p.received_at_ms >= cutoff)
            .collect();

        if !recent_probes.is_empty() {
            // Calculate delay using signed arithmetic to handle clock skew
            let delays: Vec<f64> = recent_probes
                .iter()
                .map(|p| (p.received_at_ms as i64 - p.sent_at_ms as i64) as f64)
                .collect();

            let avg_delay = delays.iter().sum::<f64>() / delays.len() as f64;
            metrics.c2s_delay_avg[i] = avg_delay;

            // Calculate jitter (std dev of delay)
            let variance =
                delays.iter().map(|d| (d - avg_delay).powi(2)).sum::<f64>() / delays.len() as f64;
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
        let recent_bulk: Vec<_> = state
            .received_bulk_bytes
            .iter()
            .filter(|b| b.received_at_ms >= cutoff)
            .collect();

        if !recent_bulk.is_empty() {
            let total_bytes: u64 = recent_bulk.iter().map(|b| b.bytes).sum();
            let time_window_sec = window_ms as f64 / 1000.0;
            metrics.c2s_throughput[i] = total_bytes as f64 / time_window_sec;
        }

        // Server-to-client throughput (from sent bulk)
        let recent_sent_bulk: Vec<_> = state
            .sent_bulk_packets
            .iter()
            .filter(|b| b.sent_at_ms >= cutoff)
            .collect();

        if !recent_sent_bulk.is_empty() {
            let total_bytes: u64 = recent_sent_bulk.iter().map(|b| b.bytes).sum();
            let time_window_sec = window_ms as f64 / 1000.0;
            metrics.s2c_throughput[i] = total_bytes as f64 / time_window_sec;
        }

        // Server-to-client metrics (from echoed probes)
        let recent_echoed_probes: Vec<_> = state
            .echoed_probes
            .iter()
            .filter(|p| p.echoed_at_ms >= cutoff)
            .collect();

        if !recent_echoed_probes.is_empty() {
            // Calculate delay using signed arithmetic to handle clock skew
            let delays: Vec<f64> = recent_echoed_probes
                .iter()
                .map(|p| (p.echoed_at_ms as i64 - p.sent_at_ms as i64) as f64)
                .collect();

            let avg_delay = delays.iter().sum::<f64>() / delays.len() as f64;
            metrics.s2c_delay_avg[i] = avg_delay;

            // Calculate jitter (std dev of delay)
            let variance =
                delays.iter().map(|d| (d - avg_delay).powi(2)).sum::<f64>() / delays.len() as f64;
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
            metrics.s2c_reorder_rate[i] =
                (reorders as f64 / recent_echoed_probes.len() as f64) * 100.0;
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

pub async fn drain_traceroute_events(
    session: Arc<ClientSession>,
    control_channel: Arc<RTCDataChannel>,
    survey_session_id: &str,
) -> i32 {
    let mut n_events = 0;

    // Check for ICMP events
    let events = session
        .packet_tracker
        .drain_events_for_conn_id(&session.conn_id)
        .await;

    for event in events {
        let hop = event.send_options.ttl.expect("TTL should be set");
        let rtt = event.icmp_received_at.duration_since(event.sent_at);
        let rtt_ms = rtt.as_secs_f64() * 1000.0;

        let hop_message = common::ControlMessage::TraceHop(common::TraceHopMessage {
            hop,
            ip_address: event.router_ip.clone(),
            rtt_ms,
            message: format_traceroute_message(hop, &event.router_ip, rtt_ms),
            conn_id: event.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
            original_src_port: event.original_src_port,
            original_dest_addr: event.original_dest_addr.clone(),
        });
        n_events += 1;

        if let Ok(msg_json) = serde_json::to_vec(&hop_message) {
            if let Err(e) = control_channel.send(&msg_json.into()).await {
                tracing::error!("Failed to send hop message: {}", e);
            }
        }
    }
    return n_events;
}

/// Run a single round of traceroute (triggered by client StartTraceroute message)
pub async fn run_single_traceroute_round(session: Arc<ClientSession>) {
    const MAX_TTL: u8 = 16;
    const TRC_SEND_INTERVAL_MS: u64 = 50; // time between TTL probes
    const TRC_DRAIN_INTERVAL_MS: u64 = 500;

    let mut n_probes_out = 0;

    tracing::info!("Running single traceroute round for session {}", session.id);

    // Get the survey session ID for messages
    let survey_session_id = session.survey_session_id.read().await.clone();

    let control_channel = {
        // Check if control channel is ready
        let channels = session.data_channels.read().await;
        let control_channel = match &channels.control {
            Some(ch) if ch.ready_state() == RTCDataChannelState::Open => ch.clone(),
            _ => {
                drop(channels);
                tracing::error!("Control channel not ready, session aborted");
                return;
            }
        };
        drop(channels);
        control_channel
    };

    for current_ttl in 1..=MAX_TTL {
        tracing::debug!(
            "Traceroute tick for session {}, TTL {}",
            session.id,
            current_ttl
        );

        // Get testprobe channel to send traceroute test probes
        let channels = session.data_channels.read().await;
        let testprobe_channel = match &channels.testprobe {
            Some(ch) if ch.ready_state() == RTCDataChannelState::Open => ch.clone(),
            _ => {
                tracing::debug!(
                    "TestProbe channel not ready for session {}, skipping",
                    session.id
                );
                drop(channels);
                continue;
            }
        };
        drop(channels);

        // Create test probe packet with specific TTL for traceroute
        let sent_at_ms = current_time_ms();
        let seq = {
            let mut state = session.measurement_state.write().await;
            let seq = state.testprobe_seq;
            state.testprobe_seq += 1;

            // Track the test probe
            let sent_testprobe = crate::state::SentProbe { seq, sent_at_ms };
            state.sent_testprobes.push_back(sent_testprobe.clone());
            state.sent_testprobes_map.insert(seq, sent_testprobe);

            // Keep only last 60 seconds of sent test probes
            let cutoff = sent_at_ms - 60_000;
            while let Some(p) = state.sent_testprobes.front() {
                if p.sent_at_ms < cutoff {
                    let old_probe = state.sent_testprobes.pop_front().unwrap();
                    state.sent_testprobes_map.remove(&old_probe.seq);
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
            track_for_ms: 5000,
            bypass_dtls: false, // Regular traceroute uses DTLS encryption
            bypass_sctp_fragmentation: false, // Use normal SCTP fragmentation
        };

        let testprobe = common::TestProbePacket {
            test_seq: seq,
            timestamp_ms: sent_at_ms,
            direction: Direction::ServerToClient,
            send_options: Some(send_options),
            conn_id: session.conn_id.clone(),
        };

        if let Ok(json) = serde_json::to_vec(&testprobe) {
            tracing::debug!(
                "Sending traceroute test probe: TTL={}, seq={}",
                current_ttl,
                seq
            );

            #[cfg(target_os = "linux")]
            let send_result = {
                use webrtc_util::UdpSendOptions;
                let options = Some(UdpSendOptions {
                    ttl: Some(current_ttl),
                    tos: None,
                    df_bit: Some(true),
                    conn_id: session.conn_id.clone(),
                    bypass_dtls: false, // Regular traceroute uses DTLS encryption
                    bypass_sctp_fragmentation: false, // Use normal SCTP fragmentation
                });
                testprobe_channel
                    .send_with_options(&json.into(), options)
                    .await
            };

            #[cfg(not(target_os = "linux"))]
            let send_result = testprobe_channel.send(&json.into()).await;

            if let Err(e) = send_result {
                tracing::error!("Failed to send traceroute test probe: {}", e);
                continue;
            }
            n_probes_out += 1;

            // Wait for ICMP response
            tokio::time::sleep(Duration::from_millis(TRC_SEND_INTERVAL_MS)).await;
            n_probes_out -= drain_traceroute_events(
                session.clone(),
                control_channel.clone(),
                &survey_session_id,
            )
            .await;
        }
    }

    let mut trace_drain_count = 2000 / TRC_DRAIN_INTERVAL_MS;
    loop {
        n_probes_out -=
            drain_traceroute_events(session.clone(), control_channel.clone(), &survey_session_id)
                .await;
        tokio::time::sleep(Duration::from_millis(TRC_DRAIN_INTERVAL_MS)).await;
        let path_ttl = {
            let mut state = session.measurement_state.read().await;
            state.path_ttl
        };
        if path_ttl.is_some() {
            tracing::debug!("traceroute - path TTL is set, stop the wait");
            break;
        }
        if trace_drain_count == 0 || n_probes_out == 0 || path_ttl.is_some() {
            break;
        }
        trace_drain_count -= 1;
    }
    let traceroute_completed_message =
        common::ControlMessage::TracerouteCompleted(common::TracerouteCompletedMessage {
            conn_id: session.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
            // packet_size,
        });

    if let Ok(msg_json) = serde_json::to_vec(&traceroute_completed_message) {
        if let Err(e) = control_channel.send(&msg_json.into()).await {
            tracing::error!("Failed to send traceroute completed message: {}", e);
        } else {
            tracing::debug!(
                "Sent traceroute completed event to client: {:?}",
                &traceroute_completed_message
            );
        }
    }

    tracing::info!(
        "Completed single traceroute round for session {}",
        session.id
    );
}

pub async fn drain_mtu_events(
    session: Arc<ClientSession>,
    control_channel: Arc<RTCDataChannel>,
    survey_session_id: &str,
) {
    // Check for ICMP events (including "Fragmentation Needed" messages)
    tracing::debug!(
        "Draining MTU ICMP event queue for conn id: {}",
        &session.conn_id
    );
    let events = session
        .packet_tracker
        .drain_events_for_conn_id(&session.conn_id)
        .await;
    tracing::debug!(
        "Draining MTU ICMP event queue for conn id: {}, got {} events",
        &session.conn_id,
        &events.len()
    );

    for event in events {
        tracing::debug!("Got an event from queue: {:?}", &event);
        let hop = event.send_options.ttl.expect("TTL should be set");
        let rtt = event.icmp_received_at.duration_since(event.sent_at);
        let rtt_ms = rtt.as_secs_f64() * 1000.0;

        // Extract MTU from ICMP "Fragmentation Needed" message if present
        let mtu = extract_mtu_from_icmp(&event.icmp_packet);
        let packet_size: u32 = event.tracked_ip_length.try_into().unwrap();

        let mtu_message = common::ControlMessage::MtuHop(common::MtuHopMessage {
            hop,
            ip_address: event.router_ip.clone(),
            rtt_ms,
            mtu,
            message: format!("MTU probe hop {} (size {})", hop, packet_size),
            conn_id: event.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
            packet_size,
        });

        if let Ok(msg_json) = serde_json::to_vec(&mtu_message) {
            if let Err(e) = control_channel.send(&msg_json.into()).await {
                tracing::error!("Failed to send MTU hop message: {}", e);
            } else {
                tracing::debug!("Sent MTU report event to client: {:?}", &mtu_message);
            }
        }
    }
}

/// Run MTU traceroute round with specified packet size
pub async fn run_mtu_traceroute_round(
    session: Arc<ClientSession>,
    packet_size: u32,
    path_ttl: i32,
    collect_timeout_ms: usize,
) {
    const TTL_SEND_INTERVAL_MS: u64 = 50;
    const TTL_DRAIN_INTERVAL_MS: u64 = 500;

    tracing::info!(
        "Running MTU traceroute round for session {} with packet_size={}",
        session.id,
        packet_size
    );

    // Get the survey session ID for messages
    let survey_session_id = session.survey_session_id.read().await.clone();

    let control_channel = {
        // Check if control channel is ready
        let channels = session.data_channels.read().await;
        let control_channel = match &channels.control {
            Some(ch) if ch.ready_state() == RTCDataChannelState::Open => ch.clone(),
            _ => {
                drop(channels);
                tracing::error!("Control channel not ready, session aborted");
                return;
            }
        };
        drop(channels);
        control_channel
    };

    for current_ttl in 1..path_ttl {
        tracing::debug!(
            "MTU traceroute tick for session {}, TTL {}, size {}",
            session.id,
            current_ttl,
            packet_size
        );

        // Get testprobe channel
        let channels = session.data_channels.read().await;
        let testprobe_channel = match &channels.testprobe {
            Some(ch) if ch.ready_state() == RTCDataChannelState::Open => ch.clone(),
            _ => {
                drop(channels);
                continue;
            }
        };
        drop(channels);

        let sent_at_ms = current_time_ms();
        let seq = {
            let mut state = session.measurement_state.write().await;
            let seq = state.testprobe_seq;
            state.testprobe_seq += 1;

            let sent_testprobe = crate::state::SentProbe { seq, sent_at_ms };
            state.sent_testprobes.push_back(sent_testprobe.clone());
            state.sent_testprobes_map.insert(seq, sent_testprobe);

            let cutoff = sent_at_ms - 60_000;
            while let Some(p) = state.sent_testprobes.front() {
                if p.sent_at_ms < cutoff {
                    let old_probe = state.sent_testprobes.pop_front().unwrap();
                    state.sent_testprobes_map.remove(&old_probe.seq);
                } else {
                    break;
                }
            }

            seq
        };

        let ttl = Some(current_ttl as u8);

        let send_options = common::SendOptions {
            ttl,
            df_bit: Some(true), // DF bit is essential for MTU discovery
            tos: None,
            flow_label: None,
            track_for_ms: 5000,
            bypass_dtls: true, // Bypass DTLS for MTU tests to control exact packet sizes
            bypass_sctp_fragmentation: true, // Bypass SCTP fragmentation for MTU tests
        };

        let testprobe = common::TestProbePacket {
            test_seq: seq,
            timestamp_ms: sent_at_ms,
            direction: Direction::ServerToClient,
            send_options: Some(send_options),
            conn_id: session.conn_id.clone(),
        };

        // Serialize and pad to packet_size
        if let Ok(mut json) = serde_json::to_vec(&testprobe) {
            // Pad the packet to the desired size
            let current_len = json.len();
            let mut target_len = packet_size as usize;
            if let Some(ip_ver) = &session.ip_version {
                if ip_ver == "ipv4" {
                    // IPv4: 20 + 8 + 28(SCTP)
                    target_len -= 20 + 8 + 28;
                } else {
                    // IPv6: 40 + 8 + 28(SCTP)
                    target_len -= 40 + 8 + 28;
                }
            }
            if current_len < target_len {
                // resize with something other than 0 to hopefully change checksum
                json.resize(target_len, 0x23);
            }

            tracing::debug!(
                "Sending MTU traceroute probe: TTL={}, seq={}, size={}",
                current_ttl,
                seq,
                json.len()
            );

            #[cfg(target_os = "linux")]
            let send_result = {
                use webrtc_util::UdpSendOptions;
                let options = Some(UdpSendOptions {
                    ttl,
                    tos: None,
                    df_bit: Some(true), // DF bit set for MTU discovery
                    conn_id: session.conn_id.clone(),
                    bypass_dtls: true, // Bypass DTLS for MTU tests to control exact packet sizes
                    bypass_sctp_fragmentation: true, // Bypass SCTP fragmentation for MTU tests
                });
                testprobe_channel
                    .send_with_options(&json.into(), options)
                    .await
            };

            #[cfg(not(target_os = "linux"))]
            let send_result = testprobe_channel.send(&json.into()).await;

            if let Err(e) = send_result {
                tracing::error!("Failed to send MTU traceroute probe: {}", e);
                continue;
            }

            tokio::time::sleep(Duration::from_millis(TTL_SEND_INTERVAL_MS)).await;
            drain_mtu_events(session.clone(), control_channel.clone(), &survey_session_id).await;
        }
    }
    let mut drain_count = collect_timeout_ms as u64 / TTL_DRAIN_INTERVAL_MS;
    loop {
        tracing::debug!(
            "Draining ICMP event queue for packet size {} conn id: {}",
            packet_size,
            &session.conn_id
        );
        drain_mtu_events(session.clone(), control_channel.clone(), &survey_session_id).await;
        if drain_count == 0 {
            break;
        }
        drain_count -= 1;
        tokio::time::sleep(Duration::from_millis(TTL_DRAIN_INTERVAL_MS)).await;
    }

    let mtu_traceroute_completed_message =
        common::ControlMessage::MtuTracerouteCompleted(common::MtuTracerouteCompletedMessage {
            conn_id: session.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
            packet_size,
        });

    if let Ok(msg_json) = serde_json::to_vec(&mtu_traceroute_completed_message) {
        if let Err(e) = control_channel.send(&msg_json.into()).await {
            tracing::error!("Failed to send traceroute completed message: {}", e);
        } else {
            tracing::debug!(
                "Sent traceroute completed event to client: {:?}",
                &mtu_traceroute_completed_message
            );
        }
    }

    tracing::info!(
        "Completed MTU traceroute round for session {} with packet_size={}",
        session.id,
        packet_size
    );
}

/// Extract MTU value from an ICMP packet
///
/// For ICMP Type 3 (Destination Unreachable), Code 4 (Fragmentation Needed),
/// the MTU of the next hop is stored in bytes 6-7 of the ICMP header.
///
/// **Note:** This function currently only handles IPv4 ICMP packets.
/// For ICMPv6, the packet structure is different and would require separate handling.
///
/// ICMP packet structure for Type 3, Code 4:
/// - Bytes 0-19: Outer IPv4 header (20 bytes for IPv4, 40 for IPv6)
/// - Byte 20: ICMP Type (3 = Destination Unreachable)
/// - Byte 21: ICMP Code (4 = Fragmentation Needed)
/// - Bytes 22-23: Checksum
/// - Bytes 24-25: Unused (should be 0)
/// - Bytes 26-27: Next-Hop MTU (big-endian u16)
/// - Bytes 28+: Original IP packet that caused the error
fn extract_mtu_from_icmp(icmp_packet: &[u8]) -> Option<u16> {
    // Need at least 28 bytes: IPv4 header (20) + ICMP header (8)
    // Note: This assumes IPv4. IPv6 would need 48 bytes minimum (40 + 8).
    if icmp_packet.len() < 28 {
        return None;
    }

    // Check ICMP Type (offset 20) - must be 3 (Destination Unreachable)
    let icmp_type = icmp_packet[20];
    if icmp_type != 3 {
        return None;
    }

    // Check ICMP Code (offset 21) - must be 4 (Fragmentation Needed)
    let icmp_code = icmp_packet[21];
    if icmp_code != 4 {
        return None;
    }

    // Extract MTU from bytes 26-27 (big-endian)
    let mtu = u16::from_be_bytes([icmp_packet[26], icmp_packet[27]]);

    // Basic validation: MTU must be > 0
    // Per RFC 791 (IPv4), minimum MTU is 68 bytes
    // Per RFC 2460 (IPv6), minimum MTU is 1280 bytes
    // We use a simple > 0 check here as the actual minimum depends on the protocol,
    // and invalid MTU values (like 0) would be caught anyway.
    if mtu > 0 {
        tracing::debug!("Extracted MTU {} from ICMP Type 3 Code 4 message", mtu);
        Some(mtu)
    } else {
        tracing::debug!("Invalid MTU value {} in ICMP message", mtu);
        None
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
pub async fn handle_testprobe_packet(session: Arc<ClientSession>, msg: DataChannelMessage) {
    if let Ok(testprobe) = serde_json::from_slice::<common::TestProbePacket>(&msg.data) {
        handle_testprobe_echo_packet(session, testprobe).await;
    } else {
        tracing::warn!("Could not deserialize testprobe message: {:?}", &msg);
    }
}

pub async fn handle_testprobe_echo_packet(
    session: Arc<ClientSession>,
    testprobe: common::TestProbePacket,
) {
    {
        tracing::info!("XXXXX handle_testprobe_echo_packet: {:?}", &testprobe);
        // Validate conn_id - ensure test probe belongs to this session
        if testprobe.conn_id != session.conn_id {
            tracing::warn!(
                "TestProbe conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                testprobe.conn_id,
                session.id,
                session.conn_id
            );
            return;
        }

        let now_ms = current_time_ms();

        let mut state = session.measurement_state.write().await;

        // Check if this is an echoed S2C test probe
        if testprobe.direction == Direction::ServerToClient {
            // This is an echoed test probe - client received our test probe and echoed it back
            tracing::debug!(
                "Received echoed S2C test probe test_seq {} from client {}",
                testprobe.test_seq,
                session.id
            );

            if let Some(opts) = testprobe.send_options {
                if let Some(ttl) = opts.ttl {
                    if state.path_ttl.is_none() {
                        tracing::debug!("Got an echoed TTL: {}, setting state TTL", &ttl);
                        state.path_ttl = Some(ttl);
                    }
                    return;
                }
            }

            // Use HashMap for O(1) lookup instead of linear search
            if let Some(sent_testprobe) = state.sent_testprobes_map.get(&testprobe.test_seq) {
                let sent_at_ms = sent_testprobe.sent_at_ms;
                state
                    .echoed_testprobes
                    .push_back(crate::state::EchoedProbe {
                        seq: testprobe.test_seq,
                        sent_at_ms,
                        echoed_at_ms: testprobe.timestamp_ms,
                    });
                tracing::debug!(
                    "Matched echoed test probe test_seq {}, delay: {}ms",
                    testprobe.test_seq,
                    testprobe.timestamp_ms as i64 - sent_at_ms as i64
                );

                // Keep only last 60 seconds of echoed test probes
                let cutoff = now_ms - 60_000;
                while let Some(p) = state.echoed_testprobes.front() {
                    if p.echoed_at_ms < cutoff {
                        state.echoed_testprobes.pop_front();
                    } else {
                        break;
                    }
                }

                // Test probe reached the client successfully
                // NOTE: We do NOT reset testprobe_seq to avoid sequence number reuse
                // while older test probes are still in flight or in the tracking deques
                // tracing::info!("🎯 Test probe reached client for session {}", session.id);
            } else {
                tracing::warn!("Received echoed test probe test_seq {} but couldn't find matching sent test probe", testprobe.test_seq);
            }
        }

        drop(state);
    }
}

/// Start the measurement probe sender for baseline measurement
/// Uses the probe channel (unreliable, unordered)
pub async fn start_measurement_probe_sender(session: Arc<ClientSession>) {
    let interval_ms = common::PROBE_INTERVAL_MS as u64;
    let mut interval = interval(Duration::from_millis(interval_ms));

    tracing::info!(
        "Starting measurement probe sender for session {} at {}pps",
        session.id,
        common::PROBE_STREAM_PPS
    );

    loop {
        interval.tick().await;

        // Check if probe streams should still be active
        let (active, seq, feedback) = {
            let mut state = session.measurement_state.write().await;
            if !state.probe_streams_active {
                tracing::info!(
                    "Stopping measurement probe sender for session {} (probe_streams_active=false)",
                    session.id
                );
                return;
            }
            let seq = state.measurement_probe_seq;
            state.measurement_probe_seq += 1;
            let feedback = state.last_feedback.clone();
            (true, seq, feedback)
        };

        if !active {
            break;
        }

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

        // Create and send measurement probe packet
        let sent_at_ms = current_time_ms();

        let probe = common::MeasurementProbePacket {
            seq,
            sent_at_ms,
            direction: Direction::ServerToClient,
            conn_id: session.conn_id.clone(),
            feedback,
        };

        if let Ok(json) = serde_json::to_vec(&probe) {
            if let Err(e) = probe_channel.send(&json.into()).await {
                tracing::error!("Failed to send measurement probe: {}", e);
                break;
            }
        }
    }
}

/// Handle incoming measurement probe packets
pub async fn handle_measurement_probe_packet(session: Arc<ClientSession>, msg: DataChannelMessage) {
    if let Ok(probe) = serde_json::from_slice::<common::MeasurementProbePacket>(&msg.data) {
        // Validate conn_id
        if probe.conn_id != session.conn_id {
            return;
        }

        let now_ms = current_time_ms();
        // Use signed arithmetic to handle clock skew between client and server
        // If client clock is ahead, delay will be negative; if behind, it will be larger than actual
        // The baseline calculation will capture the clock offset, and deviations will be meaningful
        let delay = (now_ms as i64 - probe.sent_at_ms as i64) as f64;

        let mut state = session.measurement_state.write().await;

        if !state.probe_streams_active {
            return;
        }

        // Store received probe
        state
            .received_measurement_probes
            .push_back(crate::state::ReceivedMeasurementProbe {
                seq: probe.seq,
                sent_at_ms: probe.sent_at_ms,
                received_at_ms: now_ms,
                feedback: probe.feedback.clone(),
            });

        // Update baseline delay (exponential moving average with outlier exclusion)
        // Only include delays within BASELINE_OUTLIER_MULTIPLIER of current baseline
        // Use absolute difference to handle negative delays due to clock skew
        let baseline = if state.baseline_delay_count > 0 {
            state.baseline_delay_sum / state.baseline_delay_count as f64
        } else {
            delay
        };

        // Use absolute difference from baseline for outlier detection
        // This works correctly even with clock skew (negative delays)
        let deviation_from_baseline = (delay - baseline).abs();
        let baseline_threshold = baseline.abs() * common::BASELINE_OUTLIER_MULTIPLIER;
        if state.baseline_delay_count < common::BASELINE_MIN_SAMPLES
            || deviation_from_baseline < baseline_threshold.max(common::BASELINE_MIN_THRESHOLD_MS)
        {
            state.baseline_delay_sum += delay;
            state.baseline_delay_count += 1;
        }

        // Update feedback for outgoing probes
        state.last_feedback.highest_seq = state.last_feedback.highest_seq.max(probe.seq);
        state.last_feedback.highest_seq_received_at_ms = now_ms;

        // Keep only last PROBE_STATS_WINDOW_MS of probes for stats calculation
        let cutoff = now_ms.saturating_sub(common::PROBE_STATS_WINDOW_MS);
        while let Some(p) = state.received_measurement_probes.front() {
            if p.received_at_ms < cutoff {
                state.received_measurement_probes.pop_front();
            } else {
                break;
            }
        }

        // Count recent probes and reorders for feedback
        let mut recent_count = 0u32;
        let mut recent_reorders = 0u32;
        let mut last_seq = 0u64;
        let feedback_cutoff = now_ms.saturating_sub(common::PROBE_FEEDBACK_WINDOW_MS);
        for p in state.received_measurement_probes.iter() {
            if p.received_at_ms >= feedback_cutoff {
                recent_count += 1;
                if p.seq < last_seq {
                    recent_reorders += 1;
                }
                last_seq = last_seq.max(p.seq);
            }
        }
        state.last_feedback.recent_count = recent_count;
        state.last_feedback.recent_reorders = recent_reorders;
    }
}

/// Start the per-second stats reporter
pub async fn start_probe_stats_reporter(session: Arc<ClientSession>) {
    let mut interval = interval(Duration::from_millis(1000)); // 1 Hz

    tracing::info!("Starting probe stats reporter for session {}", session.id);

    loop {
        interval.tick().await;

        // Check if probe streams should still be active
        {
            let state = session.measurement_state.read().await;
            if !state.probe_streams_active {
                tracing::info!(
                    "Stopping probe stats reporter for session {} (probe_streams_active=false)",
                    session.id
                );
                return;
            }
        }

        // Calculate stats from received probes
        let stats = calculate_probe_stream_stats(&session).await;

        // Get survey session ID
        let survey_session_id = session.survey_session_id.read().await.clone();

        // Get client-reported S2C stats
        let s2c_stats = {
            let state = session.measurement_state.read().await;
            state.client_reported_s2c_stats.clone().unwrap_or_default()
        };

        // Create stats report
        let report = common::ControlMessage::ProbeStats(common::ProbeStatsReport {
            conn_id: session.conn_id.clone(),
            survey_session_id,
            timestamp_ms: current_time_ms(),
            c2s_stats: stats, // C2S stats are what the server measures
            s2c_stats,        // S2C stats come from client reports
        });

        // Send stats report on control channel
        let channels = session.data_channels.read().await;
        if let Some(control) = &channels.control {
            if control.ready_state() == RTCDataChannelState::Open {
                if let Ok(msg_json) = serde_json::to_vec(&report) {
                    if let Err(e) = control.send(&msg_json.into()).await {
                        tracing::error!("Failed to send probe stats report: {}", e);
                    }
                }
            }
        }
    }
}

/// Calculate probe stream stats from received measurement probes
async fn calculate_probe_stream_stats(session: &Arc<ClientSession>) -> common::DirectionStats {
    let state = session.measurement_state.read().await;

    let now_ms = current_time_ms();
    let stats_cutoff = now_ms.saturating_sub(common::PROBE_FEEDBACK_WINDOW_MS);

    // Filter to probes received in the last PROBE_FEEDBACK_WINDOW_MS
    let recent_probes: Vec<_> = state
        .received_measurement_probes
        .iter()
        .filter(|p| p.received_at_ms >= stats_cutoff)
        .collect();

    if recent_probes.is_empty() {
        return common::DirectionStats::default();
    }

    let baseline = if state.baseline_delay_count > 0 {
        state.baseline_delay_sum / state.baseline_delay_count as f64
    } else {
        0.0
    };

    // Calculate delay deviations from baseline
    // Use signed arithmetic to handle clock skew between client and server
    let mut delay_deviations: Vec<f64> = recent_probes
        .iter()
        .map(|p| {
            let delay = (p.received_at_ms as i64 - p.sent_at_ms as i64) as f64;
            delay - baseline
        })
        .collect();

    delay_deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Calculate percentiles
    let len = delay_deviations.len();
    let p50_idx = len / 2;
    let p99_idx = (len * 99) / 100;

    let delay_deviation_ms = [
        delay_deviations[p50_idx],                 // 50th percentile
        delay_deviations[p99_idx.min(len - 1)],    // 99th percentile
        *delay_deviations.first().unwrap_or(&0.0), // min
        *delay_deviations.last().unwrap_or(&0.0),  // max
    ];

    // Calculate jitter (consecutive delay differences)
    // Use signed arithmetic to handle clock skew
    let mut jitters: Vec<f64> = Vec::new();
    let mut prev_delay: Option<f64> = None;
    for p in &recent_probes {
        let delay = (p.received_at_ms as i64 - p.sent_at_ms as i64) as f64;
        if let Some(prev) = prev_delay {
            jitters.push((delay - prev).abs());
        }
        prev_delay = Some(delay);
    }

    jitters.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let jitter_len = jitters.len().max(1);
    let jitter_ms = if jitters.is_empty() {
        [0.0; 4]
    } else {
        [
            jitters[jitter_len / 2],
            jitters[(jitter_len * 99) / 100],
            *jitters.first().unwrap_or(&0.0),
            *jitters.last().unwrap_or(&0.0),
        ]
    };

    // Calculate loss rate
    let min_seq = recent_probes.iter().map(|p| p.seq).min().unwrap_or(0);
    let max_seq = recent_probes.iter().map(|p| p.seq).max().unwrap_or(0);
    let expected = (max_seq.saturating_sub(min_seq) + 1) as f64;
    let received = recent_probes.len() as f64;
    let loss_rate = if expected > 0.0 {
        ((expected - received) / expected * 100.0).max(0.0)
    } else {
        0.0
    };

    // Calculate reorder rate
    let mut reorders = 0;
    let mut max_seq_seen = 0u64;
    for p in &recent_probes {
        if p.seq < max_seq_seen {
            reorders += 1;
        }
        max_seq_seen = max_seq_seen.max(p.seq);
    }
    let reorder_rate = if recent_probes.len() > 0 {
        (reorders as f64 / recent_probes.len() as f64) * 100.0
    } else {
        0.0
    };

    common::DirectionStats {
        delay_deviation_ms,
        rtt_ms: [0.0; 4], // RTT requires echo, not calculated here
        jitter_ms,
        loss_rate,
        reorder_rate,
        probe_count: recent_probes.len() as u32,
        baseline_delay_ms: baseline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_conn_id() {
        // Test empty string
        assert_eq!(hash_conn_id(""), 0);

        // Test that same conn_id always produces same hash
        let conn_id = "550e8400-e29b-41d4-a716-446655440000";
        let hash1 = hash_conn_id(conn_id);
        let hash2 = hash_conn_id(conn_id);
        assert_eq!(hash1, hash2);

        // Test that hash is in expected range [0, CONN_ID_HASH_RANGE-1]
        assert!(hash1 < CONN_ID_HASH_RANGE);

        // Test different conn_ids
        let conn_id2 = "123e4567-e89b-12d3-a456-426614174000";
        let hash3 = hash_conn_id(conn_id2);
        assert!(hash3 < CONN_ID_HASH_RANGE);
    }

    #[test]
    fn test_probe_length_uniqueness() {
        // Verify that with coprime numbers (CONN_ID_MULTIPLIER, HOP_MULTIPLIER),
        // we get unique lengths for different conn_id and hop combinations
        let mut lengths = std::collections::HashSet::new();

        // Test with different conn_id values (0..CONN_ID_HASH_RANGE) and hops (1-30)
        for conn_id_hash in 0..CONN_ID_HASH_RANGE {
            for hop in 1..=30 {
                let length =
                    BASE_PROBE_SIZE + (conn_id_hash * CONN_ID_MULTIPLIER) + (hop * HOP_MULTIPLIER);
                lengths.insert(length);
            }
        }

        // We should have CONN_ID_HASH_RANGE * 30 unique lengths
        let expected_count = CONN_ID_HASH_RANGE * 30;
        assert_eq!(
            lengths.len(),
            expected_count,
            "All probe lengths should be unique: expected {}, got {}",
            expected_count,
            lengths.len()
        );
    }

    #[test]
    fn test_current_time_ms() {
        let t1 = current_time_ms();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = current_time_ms();
        assert!(t2 > t1);
        assert!(t2 - t1 >= 10);
    }

    #[tokio::test]
    async fn test_testprobe_sequence_separate_from_probe() {
        use crate::state::{ClientSession, DataChannels, MeasurementState};
        use std::sync::Arc;
        use tokio::sync::RwLock;

        // Create a mock session
        let state = Arc::new(RwLock::new(MeasurementState::new()));

        // Verify initial state
        {
            let s = state.read().await;
            assert_eq!(s.probe_seq, 0);
            assert_eq!(s.testprobe_seq, 0);
        }

        // Simulate sending probes
        {
            let mut s = state.write().await;
            s.probe_seq += 1;
            s.probe_seq += 1;
        }

        // Simulate sending testprobes
        {
            let mut s = state.write().await;
            s.testprobe_seq += 1;
            s.testprobe_seq += 1;
            s.testprobe_seq += 1;
        }

        // Verify they maintain separate sequence spaces
        {
            let s = state.read().await;
            assert_eq!(s.probe_seq, 2);
            assert_eq!(s.testprobe_seq, 3);
        }
    }
}
