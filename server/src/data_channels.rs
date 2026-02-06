use crate::measurements;
use crate::state::ClientSession;
use common::ClientMetrics;
use std::sync::Arc;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::RTCPeerConnection;

// Mode constants for measurement type
const MODE_TRACEROUTE: &str = "traceroute";
const MODE_MEASUREMENT: &str = "measurement";

// Default measuring time in milliseconds (10000 seconds = effectively unlimited)
const DEFAULT_MEASURING_TIME_MS: u64 = 10_000_000;

pub async fn setup_data_channel_handlers(
    peer: &Arc<RTCPeerConnection>,
    session: Arc<ClientSession>,
) {
    let session_clone = session.clone();

    peer.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let label = dc.label().to_string();
        let session = session_clone.clone();

        // tracing::info!("PRE Client {} opened data channel: {}", session.id, label);
        Box::pin(async move {
            tracing::info!("Client {} opened data channel: {}", session.id, label);

            // Store the data channel
            let mut chans = session.data_channels.write().await;
            match label.as_str() {
                "probe" => {
                    chans.probe = Some(dc.clone());
                }
                "bulk" => {
                    chans.bulk = Some(dc.clone());
                }
                "control" => {
                    chans.control = Some(dc.clone());
                }
                "testprobe" => {
                    chans.testprobe = Some(dc.clone());
                    tracing::info!("TestProbe channel registered for client {}", session.id);
                }
                _ => tracing::warn!("Unknown data channel: {}", label),
            }
            let all_channels_ready = chans.all_ready();
            let control_channel = chans.control.clone();
            drop(chans);

            // Set up message handler
            let session_clone = session.clone();
            let label_clone = label.clone();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let session = session_clone.clone();
                let label = label_clone.clone();
                tracing::info!("XXXXX {}", &label);
                Box::pin(async move {
                    handle_message(session, &label, msg).await;
                })
            }));

            if all_channels_ready {
                // Send ServerSideReady message to client
                tracing::info!(
                    "All channels ready for session {}, sending ServerSideReady",
                    session.id
                );

                if let Some(control) = control_channel {
                    let survey_session_id = session.survey_session_id.read().await.clone();
                    let ready_msg =
                        common::ControlMessage::ServerSideReady(common::ServerSideReadyMessage {
                            conn_id: session.conn_id.clone(),
                            survey_session_id,
                        });

                    if let Ok(msg_json) = serde_json::to_vec(&ready_msg) {
                        if let Err(e) = control.send(&msg_json.into()).await {
                            tracing::error!("Failed to send ServerSideReady message: {}", e);
                        } else {
                            tracing::info!(
                                "Sent ServerSideReady for session {} (conn_id: {})",
                                session.id,
                                session.conn_id
                            );
                        }
                    }
                }
            }
        })
    }));
}

async fn handle_message(session: Arc<ClientSession>, channel: &str, msg: DataChannelMessage) {
    tracing::info!(
        "Client {} received message on {}: {} bytes",
        session.id,
        channel,
        msg.data.len()
    );

    match channel {
        "probe" => handle_probe_message(session, msg).await,
        "bulk" => handle_bulk_message(session, msg).await,
        "control" => handle_control_message(session, msg).await,
        "testprobe" => handle_testprobe_message(session, msg).await,
        _ => {}
    }
}

async fn handle_probe_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    // Try parsing as MeasurementProbePacket first if probe streams are active
    {
        let state = session.measurement_state.read().await;
        if state.probe_streams_active {
            drop(state);
            if let Ok(_) = serde_json::from_slice::<common::MeasurementProbePacket>(&msg.data) {
                // This is a measurement probe
                measurements::handle_measurement_probe_packet(session, msg).await;
                return;
            }
        }
    }
    // Fall back to regular probe handling
    measurements::handle_probe_packet(session, msg).await;
}

async fn handle_bulk_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_bulk_packet(session, msg).await;
}

async fn handle_control_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    tracing::debug!(
        "Control message from {} ({} bytes)",
        session.id,
        msg.data.len()
    );

    // Log the raw message for debugging (first 200 bytes)
    if let Ok(msg_str) = std::str::from_utf8(&msg.data) {
        let preview = if msg_str.len() > 200 {
            &msg_str[..200]
        } else {
            msg_str
        };
        tracing::debug!(
            "Control message content (session {}): {}",
            session.id,
            preview
        );
    }

    // Parse the message using the ControlMessage enum
    let control_msg = match serde_json::from_slice::<common::ControlMessage>(&msg.data) {
        Ok(msg) => msg,
        Err(e) => {
            tracing::warn!(
                "Failed to parse control message from session {} ({} bytes): {}. Message preview: {}",
                session.id,
                msg.data.len(),
                e,
                std::str::from_utf8(&msg.data)
                    .map(|s| if s.len() > 100 { &s[..100] } else { s })
                    .unwrap_or("<binary data>")
            );
            return;
        }
    };

    // Handle the message based on its type
    match control_msg {
        common::ControlMessage::TestProbeMessageEcho(msg) => {
            measurements::handle_testprobe_echo_packet(session, msg).await;
        }
        common::ControlMessage::TracerouteCompleted(_) => {
            tracing::warn!(
                "TracerouteCompleted should not come from client! id {}",
                session.conn_id
            )
        }
        common::ControlMessage::MtuTracerouteCompleted(_) => {
            tracing::warn!(
                "MtuTracerouteCompleted should not come from client! id {}",
                session.conn_id
            )
        }
        common::ControlMessage::StartSurveySession(start_survey_msg) => {
            if start_survey_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "StartSurveySessionMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    start_survey_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            // Store the survey session ID
            {
                let mut survey_id = session.survey_session_id.write().await;
                *survey_id = start_survey_msg.survey_session_id.clone();
            }

            // Store the magic key if provided, or extract from session ID
            if let Some(ref magic_key) = start_survey_msg.magic_key {
                let mut mk = session.magic_key.write().await;
                *mk = Some(magic_key.clone());
            } else if let Some(extracted) = extract_magic_key_from_session_id(
                &start_survey_msg.survey_session_id,
            ) {
                let mut mk = session.magic_key.write().await;
                *mk = Some(extracted);
            }

            // Create database session record if session manager is available
            if let Some(session_manager) = &session.session_manager {
                // Magic key is required for proper database tracking
                // If not provided by client, extract from the survey session ID format:
                // "survey_{magic_key}_{timestamp}_{uuid}" (hyphens encoded as underscores)
                let magic_key_owned = match start_survey_msg.magic_key.as_deref() {
                    Some(key) => key.to_string(),
                    None => {
                        extract_magic_key_from_session_id(
                            &start_survey_msg.survey_session_id,
                        )
                        .unwrap_or_else(|| {
                            tracing::warn!(
                                "Could not extract magic_key from session ID {}, using 'unknown'",
                                start_survey_msg.survey_session_id
                            );
                            "unknown".to_string()
                        })
                    }
                };
                let magic_key = &magic_key_owned;
                // TODO: Extract user_login from authentication context when available
                // This is deferred to a future issue as it requires passing auth state through the data channel flow
                if let Err(e) = session_manager
                    .create_session(
                        &start_survey_msg.survey_session_id,
                        magic_key,
                        None, // user_login - deferred to future issue
                    )
                    .await
                {
                    tracing::error!("Failed to create survey session record: {}", e);
                } else {
                    tracing::info!(
                        "Created survey session record: {} (magic_key: {})",
                        start_survey_msg.survey_session_id,
                        magic_key
                    );
                }
            }

            // Register with capture service for survey-specific pcap downloads
            // We need the peer address which should be available after connection is established
            if let Some(capture_service) = &session.capture_service {
                let peer_addr = session.peer_address.lock().await;
                if let Some((ip_str, port)) = peer_addr.as_ref() {
                    if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
                        let client_addr = std::net::SocketAddr::new(ip, *port);
                        // Use a default server port - we don't have the exact server port here
                        // but the capture service will match by client address
                        capture_service.register_session(
                            client_addr,
                            0, // Server port not strictly needed for client addr lookup
                            start_survey_msg.survey_session_id.clone(),
                        );
                        tracing::info!(
                            "Registered session {} with capture service: client={}, survey_session_id={}",
                            session.id, client_addr, start_survey_msg.survey_session_id
                        );
                    }
                } else {
                    tracing::debug!(
                        "Peer address not yet available for session {}, capture registration deferred",
                        session.id
                    );
                }
            }

            // Capture DTLS keys for the survey session
            // This allows decryption of the pcap in Wireshark
            if let Some(keylog_service) = &session.keylog_service {
                tracing::info!("DEBUG: keylog_service acquired");
                // Get the DTLS transport from the peer connection
                let dtls_transport = session.peer_connection.dtls_transport();
                if let Some(key_log_data) = dtls_transport.get_key_log_data().await {
                    tracing::info!("DEBUG: DTLS keys for survey: {:?}", &key_log_data);
                    // Store the keys with the survey session ID
                    // The SSLKEYLOGFILE format requires CLIENT_RANDOM, which from the
                    // server's perspective is the remote_random (client's random value)
                    keylog_service.add_keylog(
                        start_survey_msg.survey_session_id.clone(),
                        key_log_data.local_random, // counter intuitively named... Client random for SSLKEYLOGFILE
                        key_log_data.master_secret,
                    );
                    tracing::info!(
                        "Captured DTLS keys for survey session {} (conn_id: {})",
                        start_survey_msg.survey_session_id,
                        session.conn_id
                    );
                } else {
                    tracing::debug!(
                        "DTLS connection not available for session {}, key capture skipped",
                        session.id
                    );
                }
            }

            let (control_channel, all_channels_ready) = {
                let mut chans = session.data_channels.write().await;
                let all_channels_ready = chans.all_ready();
                let control_channel = chans.control.clone();
                drop(chans);
                (control_channel, all_channels_ready)
            };

            if let Some(control) = control_channel {
                if all_channels_ready {
                    let survey_session_id = session.survey_session_id.read().await.clone();
                    let ready_msg =
                        common::ControlMessage::ServerSideReady(common::ServerSideReadyMessage {
                            conn_id: session.conn_id.clone(),
                            survey_session_id,
                        });

                    if let Ok(msg_json) = serde_json::to_vec(&ready_msg) {
                        if let Err(e) = control.send(&msg_json.into()).await {
                            tracing::error!("Failed to send ServerSideReady message: {}", e);
                        } else {
                            tracing::info!(
                                "Sent ServerSideReady for session {} (conn_id: {})",
                                session.id,
                                session.conn_id
                            );
                        }
                    }
                }
            }

            tracing::info!(
                "Started survey session {} for connection {}",
                start_survey_msg.survey_session_id,
                session.conn_id
            );
        }

        common::ControlMessage::StartTraceroute(start_msg) => {
            if start_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "StartTracerouteMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    start_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            // Update survey session ID if provided
            if !start_msg.survey_session_id.is_empty() {
                let mut survey_id = session.survey_session_id.write().await;
                *survey_id = start_msg.survey_session_id.clone();
            }

            tracing::info!(
                "Received start traceroute request for session {} (survey: {})",
                session.id,
                start_msg.survey_session_id
            );

            // Trigger a single round of traceroute
            let session_clone = session.clone();
            tokio::spawn(async move {
                measurements::run_single_traceroute_round(session_clone).await;
            });
        }

        common::ControlMessage::StartMtuTraceroute(mtu_msg) => {
            if mtu_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "StartMtuTracerouteMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    mtu_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            // Update survey session ID if provided
            if !mtu_msg.survey_session_id.is_empty() {
                let mut survey_id = session.survey_session_id.write().await;
                *survey_id = mtu_msg.survey_session_id.clone();
            }

            tracing::info!(
                "Received MTU traceroute request for session {} with packet_size={}",
                session.id,
                mtu_msg.packet_size
            );

            // Trigger MTU traceroute with the specified packet size
            let session_clone = session.clone();
            let packet_size = mtu_msg.packet_size;
            tokio::spawn(async move {
                measurements::run_mtu_traceroute_round(
                    session_clone,
                    packet_size,
                    mtu_msg.path_ttl,
                    mtu_msg.collect_timeout_ms,
                )
                .await;
            });
        }

        common::ControlMessage::GetMeasuringTime(get_time_msg) => {
            if get_time_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "GetMeasuringTimeMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    get_time_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            tracing::info!(
                "Received GetMeasuringTime request for session {}",
                session.id
            );

            // Send back the measuring time response
            let channels = session.data_channels.read().await;
            if let Some(control) = &channels.control {
                if control.ready_state() == RTCDataChannelState::Open {
                    let survey_session_id = session.survey_session_id.read().await.clone();
                    let response = common::ControlMessage::MeasuringTimeResponse(
                        common::MeasuringTimeResponseMessage {
                            conn_id: session.conn_id.clone(),
                            survey_session_id,
                            max_duration_ms: DEFAULT_MEASURING_TIME_MS,
                        },
                    );

                    if let Ok(msg_json) = serde_json::to_vec(&response) {
                        if let Err(e) = control.send(&msg_json.into()).await {
                            tracing::error!("Failed to send MeasuringTimeResponse: {}", e);
                        } else {
                            tracing::info!(
                                "Sent MeasuringTimeResponse: {}ms for session {}",
                                DEFAULT_MEASURING_TIME_MS,
                                session.id
                            );
                        }
                    }
                }
            }
        }

        common::ControlMessage::StartServerTraffic(start_traffic_msg) => {
            if start_traffic_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "StartServerTrafficMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    start_traffic_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            // Update survey session ID if provided
            if !start_traffic_msg.survey_session_id.is_empty() {
                let mut survey_id = session.survey_session_id.write().await;
                *survey_id = start_traffic_msg.survey_session_id.clone();
            }

            tracing::info!(
                "Received StartServerTraffic for session {} (survey: {})",
                session.id,
                start_traffic_msg.survey_session_id
            );

            // Set traffic_active flag and clear metrics
            {
                let mut state = session.measurement_state.write().await;
                state.traffic_active = true;

                // Clear measurement data for fresh start
                state.received_probes.clear();
                state.received_bulk_bytes.clear();
                state.sent_bulk_packets.clear();
                state.sent_probes.clear();
                state.sent_probes_map.clear();
                state.echoed_probes.clear();
                tracing::info!(
                    "Cleared server-side metrics for measurement phase (session {})",
                    session.id
                );
            }

            // Reset ClientMetrics
            {
                let mut metrics = session.metrics.write().await;
                *metrics = ClientMetrics::default();
            }

            // Start the probe and bulk senders for measurement phase
            tracing::info!(
                "Starting probe and bulk senders for measurement phase (session {})",
                session.id
            );

            let session_for_probe = session.clone();
            tokio::spawn(async move {
                measurements::start_probe_sender(session_for_probe).await;
            });

            let session_for_bulk = session.clone();
            tokio::spawn(async move {
                measurements::start_bulk_sender(session_for_bulk).await;
            });
        }

        common::ControlMessage::StopServerTraffic(stop_traffic_msg) => {
            if stop_traffic_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "StopServerTrafficMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    stop_traffic_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            tracing::info!(
                "Received StopServerTraffic for session {} (survey: {})",
                session.id,
                stop_traffic_msg.survey_session_id
            );

            // Set traffic_active flag to false to stop senders
            {
                let mut state = session.measurement_state.write().await;
                state.traffic_active = false;
            }

            tracing::info!("Stopped server traffic for session {}", session.id);
        }

        common::ControlMessage::StartProbeStreams(start_msg) => {
            if start_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "StartProbeStreamsMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    start_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            // Update survey session ID if provided
            if !start_msg.survey_session_id.is_empty() {
                let mut survey_id = session.survey_session_id.write().await;
                *survey_id = start_msg.survey_session_id.clone();
            }

            tracing::info!(
                "Received StartProbeStreams for session {} (survey: {})",
                session.id,
                start_msg.survey_session_id
            );

            // Set probe_streams_active flag
            {
                let mut state = session.measurement_state.write().await;
                state.probe_streams_active = true;
                // Clear previous probe data for fresh measurement
                state.measurement_probe_seq = 0;
                state.received_measurement_probes.clear();
                state.probe_stats.clear();
                // Reset baseline calculation for fresh measurement
                state.baseline_delay_sum = 0.0;
                state.baseline_delay_count = 0;
                state.last_feedback = common::ProbeFeedback::default();
                state.client_reported_s2c_stats = None;
                tracing::info!("Started probe streams for session {}", session.id);
            }

            // Start the probe stream sender
            let session_for_probe = session.clone();
            tokio::spawn(async move {
                measurements::start_measurement_probe_sender(session_for_probe).await;
            });

            // Start the stats reporter (once per second)
            let session_for_stats = session.clone();
            tokio::spawn(async move {
                measurements::start_probe_stats_reporter(session_for_stats).await;
            });
        }

        common::ControlMessage::StopProbeStreams(stop_msg) => {
            if stop_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "StopProbeStreamsMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    stop_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            tracing::info!(
                "Received StopProbeStreams for session {} (survey: {})",
                session.id,
                stop_msg.survey_session_id
            );

            // Set probe_streams_active flag to false to stop senders
            {
                let mut state = session.measurement_state.write().await;
                state.probe_streams_active = false;
            }

            tracing::info!("Stopped probe streams for session {}", session.id);
        }

        common::ControlMessage::ProbeStats(stats_msg) => {
            // Client is reporting its calculated stats - store them for dashboard
            if stats_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "ProbeStatsReport conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    stats_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            tracing::debug!(
                "Received ProbeStats from client for session {}: c2s_loss={:.2}%, s2c_loss={:.2}%",
                session.id,
                stats_msg.c2s_stats.loss_rate,
                stats_msg.s2c_stats.loss_rate
            );

            // Store client-reported S2C stats
            {
                let mut state = session.measurement_state.write().await;
                state.client_reported_s2c_stats = Some(stats_msg.s2c_stats);
            }
        }

        common::ControlMessage::StopTraceroute(stop_msg) => {
            // Validate conn_id - ensure message belongs to this session
            if stop_msg.conn_id != session.conn_id {
                tracing::warn!(
                    "StopTracerouteMessage conn_id mismatch: received '{}' but session {} expects '{}', ignoring",
                    stop_msg.conn_id, session.id, session.conn_id
                );
                return;
            }

            tracing::info!(
                "Received stop traceroute request for session {}",
                session.id
            );

            // Set the stop flag and clear metrics for measurement phase
            {
                let mut state = session.measurement_state.write().await;
                state.stop_traceroute = true;

                // Clear measurement data accumulated during traceroute phase
                state.received_probes.clear();
                state.received_bulk_bytes.clear();
                state.sent_bulk_packets.clear();
                state.sent_probes.clear();
                state.sent_probes_map.clear();
                state.echoed_probes.clear();
                // Note: We don't clear sent_testprobes or testprobe_seq as they're used for traceroute
                tracing::info!("Cleared server-side metrics for session {}", session.id);
            }

            // Also reset the ClientMetrics
            {
                let mut metrics = session.metrics.write().await;
                *metrics = ClientMetrics::default();
                tracing::info!("Reset ClientMetrics for session {}", session.id);
            }

            tracing::info!(
                "Traceroute stop flag set and metrics cleared for session {}",
                session.id
            );
        }

        // Server-to-client messages (not expected here but included for completeness)
        common::ControlMessage::ServerSideReady(_)
        | common::ControlMessage::TraceHop(_)
        | common::ControlMessage::MtuHop(_)
        | common::ControlMessage::MeasuringTimeResponse(_) => {
            tracing::warn!(
                "Received unexpected server-to-client message type on control channel for session {}",
                session.id
            );
        }
    }
}

async fn handle_testprobe_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_testprobe_packet(session, msg).await;
}

/// Extract magic key from survey session ID format: "survey_{magic_key}_{timestamp}_{uuid}"
///
/// Magic key hyphens are encoded as underscores in the session ID, so we reconstruct
/// them by joining the middle parts with hyphens.
fn extract_magic_key_from_session_id(session_id: &str) -> Option<String> {
    if !session_id.starts_with("survey_") {
        return None;
    }
    let parts: Vec<&str> = session_id.split('_').collect();
    if parts.len() < 4 {
        return None;
    }
    // Parts: ["survey", key_part1, ..., key_partN, timestamp, uuid]
    // Verify the second-to-last part is a valid timestamp (numeric)
    let timestamp_idx = parts.len() - 2;
    if parts[timestamp_idx].parse::<u64>().is_err() {
        return None;
    }
    let magic_key_parts = &parts[1..timestamp_idx];
    if magic_key_parts.is_empty() {
        return None;
    }
    Some(magic_key_parts.join("-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_magic_key_simple() {
        let session_id = "survey_MYKEY_1234567890_a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        assert_eq!(
            extract_magic_key_from_session_id(session_id),
            Some("MYKEY".to_string())
        );
    }

    #[test]
    fn test_extract_magic_key_with_hyphens() {
        // Magic key "SURVEY-001" is encoded as "SURVEY_001" in the session ID
        let session_id = "survey_SURVEY_001_1234567890_a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        assert_eq!(
            extract_magic_key_from_session_id(session_id),
            Some("SURVEY-001".to_string())
        );
    }

    #[test]
    fn test_extract_magic_key_multi_part() {
        let session_id =
            "survey_MY_LONG_KEY_1234567890_a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        assert_eq!(
            extract_magic_key_from_session_id(session_id),
            Some("MY-LONG-KEY".to_string())
        );
    }

    #[test]
    fn test_extract_magic_key_invalid_format() {
        assert_eq!(extract_magic_key_from_session_id("not_a_survey_id"), None);
        assert_eq!(extract_magic_key_from_session_id("survey_"), None);
        assert_eq!(extract_magic_key_from_session_id(""), None);
    }

    #[test]
    fn test_extract_magic_key_no_timestamp() {
        // If the second-to-last part is not numeric, it should fail
        assert_eq!(
            extract_magic_key_from_session_id("survey_KEY_notanumber_uuid"),
            None
        );
    }
}
