use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
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

    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
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

    Ok(answer.sdp)
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
