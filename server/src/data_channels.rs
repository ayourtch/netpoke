use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::state::DataChannels;

pub async fn setup_data_channel_handlers(
    peer: &Arc<RTCPeerConnection>,
    channels: Arc<RwLock<DataChannels>>,
    client_id: String,
) {
    let channels_clone = channels.clone();
    let client_id_clone = client_id.clone();

    peer.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let label = dc.label().to_string();
        let channels = channels_clone.clone();
        let client_id = client_id_clone.clone();

        Box::pin(async move {
            tracing::info!("Client {} opened data channel: {}", client_id, label);

            // Store the data channel
            let mut chans = channels.write().await;
            match label.as_str() {
                "probe" => chans.probe = Some(dc.clone()),
                "bulk" => chans.bulk = Some(dc.clone()),
                "control" => chans.control = Some(dc.clone()),
                _ => tracing::warn!("Unknown data channel: {}", label),
            }
            drop(chans);

            // Set up message handler
            let client_id_clone = client_id.clone();
            let label_clone = label.clone();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                let client_id = client_id_clone.clone();
                let label = label_clone.clone();
                Box::pin(async move {
                    handle_message(&client_id, &label, msg).await;
                })
            }));
        })
    }));
}

async fn handle_message(client_id: &str, channel: &str, msg: DataChannelMessage) {
    tracing::debug!("Client {} received message on {}: {} bytes",
                   client_id, channel, msg.data.len());

    match channel {
        "probe" => handle_probe_message(client_id, msg).await,
        "bulk" => handle_bulk_message(client_id, msg).await,
        "control" => handle_control_message(client_id, msg).await,
        _ => {}
    }
}

async fn handle_probe_message(client_id: &str, msg: DataChannelMessage) {
    // Will implement in next task
    tracing::trace!("Probe message from {}", client_id);
}

async fn handle_bulk_message(client_id: &str, msg: DataChannelMessage) {
    // Will implement in next task
    tracing::trace!("Bulk message from {}: {} bytes", client_id, msg.data.len());
}

async fn handle_control_message(client_id: &str, msg: DataChannelMessage) {
    // Will implement in next task
    tracing::trace!("Control message from {}", client_id);
}
