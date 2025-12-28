use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use std::sync::Arc;
use crate::state::ClientSession;
use crate::measurements;
use common::ClientMetrics;

// Mode constants for measurement type
const MODE_TRACEROUTE: &str = "traceroute";
const MODE_MEASUREMENT: &str = "measurement";

pub async fn setup_data_channel_handlers(
    peer: &Arc<RTCPeerConnection>,
    session: Arc<ClientSession>,
) {
    let session_clone = session.clone();

    peer.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let label = dc.label().to_string();
        let session = session_clone.clone();

        Box::pin(async move {
            tracing::info!("Client {} opened data channel: {}", session.id, label);

            // Store the data channel
            let mut chans = session.data_channels.write().await;
            match label.as_str() {
                "probe" => {
                    chans.probe = Some(dc.clone());
                    // Only start probe sender if mode is NOT traceroute
                    // In traceroute mode, probe sender starts after stop_traceroute message
                    let should_start_now = session.mode.as_deref() != Some(MODE_TRACEROUTE);
                    if should_start_now {
                        tracing::info!("Starting probe sender immediately for session {} (mode: {:?})", session.id, session.mode);
                        let session_clone = session.clone();
                        tokio::spawn(async move {
                            measurements::start_probe_sender(session_clone).await;
                        });
                    } else {
                        tracing::info!("Delaying probe sender for session {} until traceroute completes (mode: {:?})", session.id, session.mode);
                    }
                },
                "bulk" => {
                    chans.bulk = Some(dc.clone());
                    // Only start bulk sender if mode is NOT traceroute
                    // In traceroute mode, bulk sender starts after stop_traceroute message
                    let should_start_now = session.mode.as_deref() != Some(MODE_TRACEROUTE);
                    if should_start_now {
                        tracing::info!("Starting bulk sender immediately for session {} (mode: {:?})", session.id, session.mode);
                        let session_clone = session.clone();
                        tokio::spawn(async move {
                            measurements::start_bulk_sender(session_clone).await;
                        });
                    } else {
                        tracing::info!("Delaying bulk sender for session {} until traceroute completes (mode: {:?})", session.id, session.mode);
                    }
                },
                "control" => {
                    chans.control = Some(dc.clone());
                    // Only start traceroute sender if mode is "traceroute"
                    let should_start_traceroute = session.mode.as_deref() == Some(MODE_TRACEROUTE);
                    if should_start_traceroute {
                        tracing::info!("Starting traceroute sender for session {} (mode: {})", session.id, MODE_TRACEROUTE);
                        let session_clone = session.clone();
                        tokio::spawn(async move {
                            measurements::start_traceroute_sender(session_clone).await;
                        });
                    } else {
                        tracing::info!("Skipping traceroute sender for session {} (mode: {:?})", session.id, session.mode);
                    }
                },
                "testprobe" => {
                    chans.testprobe = Some(dc.clone());
                    tracing::info!("TestProbe channel registered for client {}", session.id);
                },
                _ => tracing::warn!("Unknown data channel: {}", label),
            }
            drop(chans);

            // Set up message handler
            let session_clone = session.clone();
            let label_clone = label.clone();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let session = session_clone.clone();
                let label = label_clone.clone();
                Box::pin(async move {
                    handle_message(session, &label, msg).await;
                })
            }));
        })
    }));
}

async fn handle_message(session: Arc<ClientSession>, channel: &str, msg: DataChannelMessage) {
    tracing::debug!("Client {} received message on {}: {} bytes",
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
    tracing::trace!("Control message from {}", session.id);
    
    // Try to parse as StopTracerouteMessage
    if let Ok(stop_msg) = serde_json::from_slice::<common::StopTracerouteMessage>(&msg.data) {
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
        
        // Now start the probe and bulk senders for measurement phase
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
}

async fn handle_testprobe_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_testprobe_packet(session, msg).await;
}
