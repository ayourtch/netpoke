use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use std::sync::Arc;
use crate::state::ClientSession;
use crate::measurements;

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
                    // Start probe sender
                    let session_clone = session.clone();
                    tokio::spawn(async move {
                        measurements::start_probe_sender(session_clone).await;
                    });
                },
                "bulk" => {
                    chans.bulk = Some(dc.clone());
                    // Start bulk sender
                    let session_clone = session.clone();
                    tokio::spawn(async move {
                        measurements::start_bulk_sender(session_clone).await;
                    });
                },
                "control" => {
                    chans.control = Some(dc.clone());
                    // Start traceroute sender
                    let session_clone = session.clone();
                    tokio::spawn(async move {
                        measurements::start_traceroute_sender(session_clone).await;
                    });
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
        _ => {}
    }
}

async fn handle_probe_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_probe_packet(session, msg).await;
}

async fn handle_bulk_message(session: Arc<ClientSession>, msg: DataChannelMessage) {
    measurements::handle_bulk_packet(session, msg).await;
}

async fn handle_control_message(session: Arc<ClientSession>, _msg: DataChannelMessage) {
    tracing::trace!("Control message from {}", session.id);
}
