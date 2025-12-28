use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use std::sync::Arc;

pub async fn create_peer_connection() -> Result<Arc<RTCPeerConnection>, Box<dyn std::error::Error>> {
    let mut media_engine = MediaEngine::default();
    let registry = Registry::new();

    let registry = register_default_interceptors(registry, &mut media_engine)?;

    // Configure SettingEngine to disable mDNS
    let mut setting_engine = SettingEngine::default();
    setting_engine.set_ice_multicast_dns_mode(webrtc::ice::mdns::MulticastDnsMode::Disabled);

    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .with_setting_engine(setting_engine)
        .build();

    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ice_transport_policy: "all".into(),
        ..Default::default()
    };

    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    Ok(peer_connection)
}

pub async fn handle_offer(
    peer: &Arc<RTCPeerConnection>,
    offer_sdp: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let offer = RTCSessionDescription::offer(offer_sdp)?;
    peer.set_remote_description(offer).await?;

    let answer = peer.create_answer(None).await?;
    peer.set_local_description(answer.clone()).await?;

    // Wait for ICE gathering to complete
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);
    peer.on_ice_gathering_state_change(Box::new(move |state| {
        let state_str = state.to_string();
        let _ = tx.try_send(state_str);
        Box::pin(async {})
    }));

    while let Some(gathering) = rx.recv().await {
        tracing::debug!("ICE gathering state: {}", gathering);
        if gathering == "complete" {
            tracing::info!("ICE gathering complete");
            break;
        }
    }

    // Get the final SDP with all ICE candidates
    let final_answer = peer.local_description().await
        .ok_or("No local description")?
        .sdp;

    Ok(final_answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_peer_connection() {
        let result = create_peer_connection().await;
        assert!(result.is_ok());
    }
}
