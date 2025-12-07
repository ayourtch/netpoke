use wasm_bindgen::prelude::*;
use web_sys::{
    RtcPeerConnection, RtcConfiguration,
    RtcDataChannelInit, RtcSessionDescriptionInit, RtcSdpType,
    RtcIceCandidateInit,
};
use js_sys;
use crate::{signaling, measurements};
use std::rc::Rc;
use std::cell::RefCell;

/// Check if we should stop polling for ICE candidates
/// Returns true when ICE gathering is complete and connection is established
async fn check_stop_polling(peer: &RtcPeerConnection) -> bool {
    // Check ICE gathering state
    let ice_gathering_state = js_sys::Reflect::get(peer, &"iceGatheringState".into())
        .ok()
        .and_then(|s| s.as_string());
    let ice_complete = ice_gathering_state.as_deref() == Some("complete");

    // Check connection state
    let connection_state = js_sys::Reflect::get(peer, &"connectionState".into())
        .ok()
        .and_then(|s| s.as_string());
    let connected = connection_state.as_deref()
        .map(|s| s == "connected" || s == "completed")
        .unwrap_or(false);

    // Stop when both ICE gathering is complete AND connection is established
    ice_complete && connected
}

pub struct WebRtcConnection {
    pub peer: RtcPeerConnection,
    pub client_id: String,
    pub state: Rc<RefCell<measurements::MeasurementState>>,
}

impl WebRtcConnection {
    pub async fn new_with_ip_version(ip_version: &str, parent_client_id: Option<String>) -> Result<Self, JsValue> {
        log::info!("Creating RTCPeerConnection for IP version: {}", ip_version);

        let mut config = RtcConfiguration::new();

        // Configure ICE servers based on IP version
        let ice_servers = js_sys::Array::new();
        let server = js_sys::Object::new();

        // Use different STUN servers based on IP version preference
        let stun_url = if ip_version.eq_ignore_ascii_case("ipv6") {
            // Try IPv6-capable STUN server first, fallback to dual-stack
            "stun:stun.l.google.com:19302"
        } else {
            // IPv4 preference
            "stun:stun.l.google.com:19302"
        };

        js_sys::Reflect::set(&server, &"urls".into(), &stun_url.into())?;
        ice_servers.push(&server);
        config.set_ice_servers(&ice_servers);

        let peer = RtcPeerConnection::new_with_configuration(&config)?;
        let state = Rc::new(RefCell::new(measurements::MeasurementState::new()));

        log::info!("Creating data channels");

        // Create probe channel (unreliable, unordered)
        let mut probe_init = RtcDataChannelInit::new();
        probe_init.set_ordered(false);
        probe_init.set_max_retransmits(0);
        let probe_channel = peer.create_data_channel_with_data_channel_dict("probe", &probe_init);
        measurements::setup_probe_channel(probe_channel, state.clone());

        // Create bulk channel (reliable, ordered)
        let bulk_init = RtcDataChannelInit::new();
        let bulk_channel = peer.create_data_channel_with_data_channel_dict("bulk", &bulk_init);
        measurements::setup_bulk_channel(bulk_channel, state.clone());

        // Create control channel (reliable, ordered)
        let control_init = RtcDataChannelInit::new();
        let control_channel = peer.create_data_channel_with_data_channel_dict("control", &control_init);
        measurements::setup_control_channel(control_channel);

        log::info!("Creating offer");

        let offer = wasm_bindgen_futures::JsFuture::from(peer.create_offer()).await?;
        let offer_sdp = js_sys::Reflect::get(&offer, &"sdp".into())?
            .as_string()
            .ok_or("No SDP in offer")?;

        let mut offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        offer_obj.set_sdp(&offer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_local_description(&offer_obj)
        ).await?;

        log::info!("Sending offer to server");

        let (client_id, parent_id_from_server, _ip_version_from_server, answer_sdp) =
            signaling::send_offer(offer_sdp, parent_client_id.clone(), Some(ip_version.to_string())).await?;

        log::info!("Received answer from server, client_id: {}", client_id);

        let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        answer_obj.set_sdp(&answer_sdp);
        log::info!("Remote answer: {:?}", &answer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_remote_description(&answer_obj)
        ).await?;

        // Set up ICE candidate event handler
        let _peer_clone = peer.clone();
        let client_id_clone = client_id.clone();
        let onicecandidate = Closure::wrap(Box::new(move |event: web_sys::Event| {
            // Check if this is an icecandidate event
            if event.type_() == "icecandidate" {
                if let Ok(candidate) = js_sys::Reflect::get(&event, &"candidate".into()) {
                    if !candidate.is_undefined() && !candidate.is_null() {
                        // Convert JsValue to JSON string
                        if let Ok(json_str) = js_sys::JSON::stringify(&candidate) {
                            let candidate_str = json_str.as_string().unwrap_or_default();

                            // Skip empty candidates (end-of-candidates)
                            if !candidate_str.trim().is_empty() && !candidate_str.contains("\"\"") {
                                let client_id = client_id_clone.clone();

                                // Send ICE candidate to server
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Err(e) = signaling::send_ice_candidate(&client_id, &candidate_str).await {
                                        log::error!("Failed to send ICE candidate: {:?}", e);
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);

        peer.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));
        onicecandidate.forget();

        // Start polling for server ICE candidates
        let peer_for_poll = peer.clone();
        let client_id_for_poll = client_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let mut poll_count = 0;
            let max_polls = 3000; // Timeout after 3000 polls (about 5 minutes at 100ms intervals)

            loop {
                // Wait 100ms between polls
                wasm_bindgen_futures::JsFuture::from(
                    js_sys::Promise::new(&mut |resolve, _| {
                        web_sys::window().unwrap()
                            .set_timeout_with_callback_and_timeout_and_arguments_0(
                                &resolve,
                                100,
                            ).unwrap();
                    })
                ).await.unwrap();

                poll_count += 1;

                // Check if we should stop polling
                let should_stop = check_stop_polling(&peer_for_poll).await || poll_count >= max_polls;

                if let Ok(candidates) = signaling::get_ice_candidates(&client_id_for_poll).await {
                    for candidate_str in candidates {
                        // Parse JSON string to extract candidate field
                        if let Ok(candidate_obj) = js_sys::JSON::parse(&candidate_str) {
                            if let Ok(candidate) = js_sys::Reflect::get(&candidate_obj, &"candidate".into()) {
                                if let Some(candidate_str) = candidate.as_string() {
                                    let candidate_init = RtcIceCandidateInit::new(&candidate_str);
                                    if let Err(e) = wasm_bindgen_futures::JsFuture::from(
                                        peer_for_poll.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&candidate_init))
                                    ).await {
                                        log::error!("Failed to add remote ICE candidate: {:?}", e);
                                    }
                                }
                            }
                        }
                    }
                }

                if should_stop {
                    log::info!("Stopping ICE candidate polling");
                    break;
                }
            }
        });

        log::info!("WebRTC connection established");

        Ok(Self { peer, client_id, state })
    }
}
