use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::ice::network_type::NetworkType;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use std::sync::Arc;
use common::IpFamily;

/// Create a new RTCPeerConnection with the specified IP family filtering.
/// 
/// # Arguments
/// 
/// * `ip_family` - Optional IP family to restrict candidate gathering:
///   - `IpFamily::IPv4` - Only gather IPv4 (UDP4) candidates
///   - `IpFamily::IPv6` - Only gather IPv6 (UDP6) candidates
///   - `IpFamily::Both` or `None` - Gather both IPv4 and IPv6 candidates (default)
pub async fn create_peer_connection(ip_family: Option<IpFamily>) -> Result<Arc<RTCPeerConnection>, Box<dyn std::error::Error>> {
    let mut media_engine = MediaEngine::default();
    let registry = Registry::new();

    let registry = register_default_interceptors(registry, &mut media_engine)?;

    // Configure SettingEngine to disable mDNS and set network types based on IP family
    let mut setting_engine = SettingEngine::default();
    setting_engine.set_ice_multicast_dns_mode(webrtc::ice::mdns::MulticastDnsMode::Disabled);
    
    // Apply IP family filtering if specified
    match ip_family {
        Some(IpFamily::IPv4) => {
            tracing::info!("Restricting ICE candidates to IPv4 only");
            setting_engine.set_network_types(vec![NetworkType::Udp4]);
        }
        Some(IpFamily::IPv6) => {
            tracing::info!("Restricting ICE candidates to IPv6 only");
            setting_engine.set_network_types(vec![NetworkType::Udp6]);
        }
        Some(IpFamily::Both) | None => {
            tracing::debug!("Using default network types (IPv4 and IPv6)");
            // Default behavior - allow both IPv4 and IPv6
        }
    }

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
        let result = create_peer_connection(None).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_peer_connection_ipv4_only() {
        let result = create_peer_connection(Some(IpFamily::IPv4)).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_peer_connection_ipv6_only() {
        let result = create_peer_connection(Some(IpFamily::IPv6)).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_peer_connection_both() {
        let result = create_peer_connection(Some(IpFamily::Both)).await;
        assert!(result.is_ok());
    }
}
