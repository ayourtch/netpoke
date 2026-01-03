use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use std::sync::Arc;
use crate::state::ClientSession;
use crate::measurements;
use common::ClientMetrics;

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
                },
                "bulk" => {
                    chans.bulk = Some(dc.clone());
                },
                "control" => {
                    chans.control = Some(dc.clone());
                },
                "testprobe" => {
                    chans.testprobe = Some(dc.clone());
                    tracing::info!("TestProbe channel registered for client {}", session.id);
                },
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
                tracing::info!("All channels ready for session {}, sending ServerSideReady", session.id);
                
                if let Some(control) = control_channel {
                    let survey_session_id = session.survey_session_id.read().await.clone();
                    let ready_msg = common::ControlMessage::ServerSideReady(
                        common::ServerSideReadyMessage {
                            conn_id: session.conn_id.clone(),
                            survey_session_id,
                        }
                    );
                    
                    if let Ok(msg_json) = serde_json::to_vec(&ready_msg) {
                        if let Err(e) = control.send(&msg_json.into()).await {
                            tracing::error!("Failed to send ServerSideReady message: {}", e);
                        } else {
                            tracing::info!("Sent ServerSideReady for session {} (conn_id: {})", session.id, session.conn_id);
                        }
                    }
                }
            }

        })
    }));
}

async fn handle_message(session: Arc<ClientSession>, channel: &str, msg: DataChannelMessage) {
    tracing::info!("Client {} received message on {}: {} bytes",
                   session.id, channel, msg.data.len());

    match channel {
        "probe" => handle_probe_message(session, msg).await,
        "bulk" => handle_bulk_message(session, msg).await,
        "control" => handle_control_message(session, msg).await,
        "testprobe" => handle_testprobe_message(session, msg).await,
        _ => {}
    }
}

async fn handle_probe_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_probe_packet(session, msg).await;
}

async fn handle_bulk_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_bulk_packet(session, msg).await;
}

async fn handle_control_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    tracing::debug!("Control message from {} ({} bytes)", session.id, msg.data.len());
    
    // Log the raw message for debugging (first 200 bytes)
    if let Ok(msg_str) = std::str::from_utf8(&msg.data) {
        let preview = if msg_str.len() > 200 {
            &msg_str[..200]
        } else {
            msg_str
        };
        tracing::debug!("Control message content (session {}): {}", session.id, preview);
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
            tracing::warn!("TracerouteCompleted should not come from client! id {}", session.conn_id)
        },
        common::ControlMessage::MtuTracerouteCompleted(_) => {
            tracing::warn!("MtuTracerouteCompleted should not come from client! id {}", session.conn_id)
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
                    let ready_msg = common::ControlMessage::ServerSideReady(
                        common::ServerSideReadyMessage {
                            conn_id: session.conn_id.clone(),
                            survey_session_id,
                        }
                    );

                    if let Ok(msg_json) = serde_json::to_vec(&ready_msg) {
                        if let Err(e) = control.send(&msg_json.into()).await {
                            tracing::error!("Failed to send ServerSideReady message: {}", e);
                        } else {
                            tracing::info!("Sent ServerSideReady for session {} (conn_id: {})", session.id, session.conn_id);
                        }
                    }
                }
            }
            
            tracing::info!("Started survey session {} for connection {}", 
                start_survey_msg.survey_session_id, session.conn_id);
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
            
            tracing::info!("Received start traceroute request for session {} (survey: {})", 
                session.id, start_msg.survey_session_id);
            
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
            
            tracing::info!("Received MTU traceroute request for session {} with packet_size={}", 
                session.id, mtu_msg.packet_size);
            
            // Trigger MTU traceroute with the specified packet size
            let session_clone = session.clone();
            let packet_size = mtu_msg.packet_size;
            tokio::spawn(async move {
                measurements::run_mtu_traceroute_round(session_clone, packet_size, mtu_msg.path_ttl, mtu_msg.collect_timeout_ms).await;
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
            
            tracing::info!("Received GetMeasuringTime request for session {}", session.id);
            
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
                        }
                    );
                    
                    if let Ok(msg_json) = serde_json::to_vec(&response) {
                        if let Err(e) = control.send(&msg_json.into()).await {
                            tracing::error!("Failed to send MeasuringTimeResponse: {}", e);
                        } else {
                            tracing::info!("Sent MeasuringTimeResponse: {}ms for session {}", 
                                DEFAULT_MEASURING_TIME_MS, session.id);
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
            
            tracing::info!("Received StartServerTraffic for session {} (survey: {})", 
                session.id, start_traffic_msg.survey_session_id);
            
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
                tracing::info!("Cleared server-side metrics for measurement phase (session {})", session.id);
            }
            
            // Reset ClientMetrics
            {
                let mut metrics = session.metrics.write().await;
                *metrics = ClientMetrics::default();
            }
            
            // Start the probe and bulk senders for measurement phase
            tracing::info!("Starting probe and bulk senders for measurement phase (session {})", session.id);
            
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
            
            tracing::info!("Received StopServerTraffic for session {} (survey: {})", 
                session.id, stop_traffic_msg.survey_session_id);
            
            // Set traffic_active flag to false to stop senders
            {
                let mut state = session.measurement_state.write().await;
                state.traffic_active = false;
            }
            
            tracing::info!("Stopped server traffic for session {}", session.id);
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
            
            tracing::info!("Received stop traceroute request for session {}", session.id);
            
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
            
            tracing::info!("Traceroute stop flag set and metrics cleared for session {}", session.id);
        }
        
        // Server-to-client messages (not expected here but included for completeness)
        common::ControlMessage::ServerSideReady(_) |
        common::ControlMessage::TraceHop(_) |
        common::ControlMessage::MtuHop(_) |
        common::ControlMessage::MeasuringTimeResponse(_) => {
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
