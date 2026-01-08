use common::IpFamily;
use std::sync::Arc;
use std::time::Duration;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::ice::network_type::NetworkType;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

/// Create a new RTCPeerConnection with the specified IP family filtering.
///
/// # Arguments
///
/// * `ip_family` - Optional IP family to restrict candidate gathering:
///   - `IpFamily::IPv4` - Only gather IPv4 (UDP4) candidates
///   - `IpFamily::IPv6` - Only gather IPv6 (UDP6) candidates
///   - `IpFamily::Both` or `None` - Gather both IPv4 and IPv6 candidates (default)
pub async fn create_peer_connection(
    ip_family: Option<IpFamily>,
) -> Result<Arc<RTCPeerConnection>, Box<dyn std::error::Error>> {
    let mut media_engine = MediaEngine::default();
    let registry = Registry::new();

    let registry = register_default_interceptors(registry, &mut media_engine)?;

    // Configure SettingEngine to disable mDNS and set network types based on IP family
    let mut setting_engine = SettingEngine::default();
    setting_engine.set_ice_multicast_dns_mode(webrtc::ice::mdns::MulticastDnsMode::Disabled);

    // Enable ICE Lite mode for server-side.
    // ICE Lite makes the server act as a 'controlled' agent that:
    // 1. Only gathers host candidates (binding to specific local IPs, not wildcards)
    // 2. Does not perform connectivity checks - relies on the client (controlling agent)
    // 3. Does not require STUN/TURN servers
    // This is appropriate when the server is directly reachable by clients.
    setting_engine.set_lite(true);

    // Configure ICE timeouts for more robust connections
    // - disconnected_timeout: 10s (default 5s) - more tolerance for temporary disconnections
    // - failed_timeout: 30s (default 25s) - give more time to recover from disconnected state
    // - keepalive_interval: 2s (default 2s) - keep connections alive with regular traffic
    setting_engine.set_ice_timeouts(
        Some(Duration::from_secs(10)), // disconnected_timeout
        Some(Duration::from_secs(30)), // failed_timeout
        Some(Duration::from_secs(2)),  // keepalive_interval
    );

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

    // ICE Lite mode: No STUN/TURN servers needed since only host candidates are gathered.
    // The server relies on the client to perform connectivity checks.
    let config = RTCConfiguration {
        ice_servers: vec![],
        ice_transport_policy: "all".into(),
        ..Default::default()
    };

    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    Ok(peer_connection)
}

/// Timeout for ICE gathering to complete (in seconds)
const ICE_GATHERING_TIMEOUT_SECS: u64 = 10;

/// Buffer size for ICE gathering state change channel.
/// Set to 16 to handle rapid state changes without blocking the callback.
const ICE_STATE_CHANNEL_BUFFER: usize = 16;

pub async fn handle_offer(
    peer: &Arc<RTCPeerConnection>,
    offer_sdp: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let offer = RTCSessionDescription::offer(offer_sdp)?;
    peer.set_remote_description(offer).await?;

    let answer = peer.create_answer(None).await?;

    // Set up ICE gathering state change callback BEFORE setting local description
    // This prevents a race condition where gathering completes before we start listening
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(ICE_STATE_CHANNEL_BUFFER);
    peer.on_ice_gathering_state_change(Box::new(move |state| {
        let state_str = state.to_string();
        tracing::debug!("ICE gathering state change: {}", state_str);
        let _ = tx.try_send(state_str);
        Box::pin(async {})
    }));

    // Now set local description - ICE gathering starts after this
    peer.set_local_description(answer.clone()).await?;

    // Wait for ICE gathering to complete with timeout
    let timeout_duration = tokio::time::Duration::from_secs(ICE_GATHERING_TIMEOUT_SECS);
    let gathering_result = tokio::time::timeout(timeout_duration, async {
        // First check if gathering is already complete
        let current_state = peer.ice_gathering_state();
        tracing::debug!("Initial ICE gathering state: {:?}", current_state);
        if current_state
            == webrtc::ice_transport::ice_gathering_state::RTCIceGatheringState::Complete
        {
            tracing::info!("ICE gathering already complete");
            return Ok::<(), String>(());
        }

        // Wait for gathering to complete via callbacks
        while let Some(gathering) = rx.recv().await {
            tracing::debug!("ICE gathering state: {}", gathering);
            if gathering == "complete" {
                tracing::info!("ICE gathering complete");
                return Ok(());
            }
        }

        // Channel closed without completing - check final state
        let final_state = peer.ice_gathering_state();
        if final_state == webrtc::ice_transport::ice_gathering_state::RTCIceGatheringState::Complete
        {
            tracing::info!("ICE gathering complete (detected after channel close)");
            Ok(())
        } else {
            tracing::warn!(
                "ICE gathering incomplete: callback channel closed before completion (state: {:?})",
                final_state
            );
            Err("ICE gathering incomplete: callback channel closed before completion".to_string())
        }
    })
    .await;

    match gathering_result {
        Ok(Ok(())) => {
            // Gathering completed successfully
        }
        Ok(Err(e)) => {
            tracing::error!("ICE gathering failed: {}", e);
            return Err(e.into());
        }
        Err(_) => {
            // Timeout - log warning but continue with whatever candidates we have
            let current_state = peer.ice_gathering_state();
            tracing::warn!(
                "ICE gathering timed out after {} seconds (state: {:?}), continuing with available candidates",
                ICE_GATHERING_TIMEOUT_SECS,
                current_state
            );
        }
    }

    // Get the final SDP with all ICE candidates
    let final_answer = peer
        .local_description()
        .await
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
