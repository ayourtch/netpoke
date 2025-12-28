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

/// Determine if an ICE candidate is IPv4 or IPv6 by parsing the candidate string
/// Returns Some("ipv4"), Some("ipv6"), or None if unable to determine
fn get_candidate_ip_version(candidate_str: &str) -> Option<String> {
    // Parse the candidate SDP attribute
    // Format: "candidate:foundation component protocol priority ip port typ type ..."
    // Example: "candidate:1234567890 1 udp 2122260223 192.168.1.100 54321 typ host"

    if let Some(candidate_part) = candidate_str.strip_prefix("candidate:") {
        let parts: Vec<&str> = candidate_part.split_whitespace().collect();
        if parts.len() >= 5 {
            let ip = parts[4]; // IP address is the 5th field (index 4)

            // Check if it contains ':' which indicates IPv6
            if ip.contains(':') {
                return Some("ipv6".to_string());
            } else if ip.contains('.') {
                return Some("ipv4".to_string());
            }
        }
    }

    None
}

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
    pub conn_id: String,
    pub state: Rc<RefCell<measurements::MeasurementState>>,
    pub control_channel: Rc<RefCell<Option<web_sys::RtcDataChannel>>>,
}

impl WebRtcConnection {
    pub async fn new_with_ip_version(ip_version: &str, parent_client_id: Option<String>) -> Result<Self, JsValue> {
        Self::new_with_ip_version_and_mode(ip_version, parent_client_id, None, None).await
    }

    pub async fn new_with_ip_version_and_mode(ip_version: &str, parent_client_id: Option<String>, mode: Option<String>, conn_id: Option<String>) -> Result<Self, JsValue> {
        // Generate a UUID for this connection if not provided
        let generated_conn_id = conn_id.unwrap_or_else(|| {
            // Generate a simple UUID-like ID using random bytes
            let random_bytes: [u8; 16] = [
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
                (js_sys::Math::random() * 256.0) as u8,
            ];
            format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                random_bytes[0], random_bytes[1], random_bytes[2], random_bytes[3],
                random_bytes[4], random_bytes[5],
                random_bytes[6], random_bytes[7],
                random_bytes[8], random_bytes[9],
                random_bytes[10], random_bytes[11], random_bytes[12], random_bytes[13], random_bytes[14], random_bytes[15])
        });
        log::info!("Creating RTCPeerConnection for IP version: {}, conn_id: {}", ip_version, generated_conn_id);

        let config = RtcConfiguration::new();

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
        let probe_init = RtcDataChannelInit::new();
        probe_init.set_ordered(false);
        probe_init.set_max_retransmits(0);
        let probe_channel = peer.create_data_channel_with_data_channel_dict("probe", &probe_init);
        measurements::setup_probe_channel(probe_channel, state.clone());

        // Create bulk channel (reliable, ordered)
        let bulk_init = RtcDataChannelInit::new();
        let bulk_channel = peer.create_data_channel_with_data_channel_dict("bulk", &bulk_init);
        measurements::setup_bulk_channel(bulk_channel, state.clone());

        // Create control channel (reliable, ordered) and store it for sending stop messages
        let control_init = RtcDataChannelInit::new();
        let control_channel = peer.create_data_channel_with_data_channel_dict("control", &control_init);
        let control_channel_ref = Rc::new(RefCell::new(Some(control_channel.clone())));
        measurements::setup_control_channel(control_channel, state.clone());

        // Create testprobe channel (unreliable, unordered) for traceroute test probes
        let testprobe_init = RtcDataChannelInit::new();
        testprobe_init.set_ordered(false);
        testprobe_init.set_max_retransmits(0);
        let testprobe_channel = peer.create_data_channel_with_data_channel_dict("testprobe", &testprobe_init);
        measurements::setup_testprobe_channel(testprobe_channel, state.clone());

        log::info!("Creating offer");

        let offer = wasm_bindgen_futures::JsFuture::from(peer.create_offer()).await?;
        let offer_sdp = js_sys::Reflect::get(&offer, &"sdp".into())?
            .as_string()
            .ok_or("No SDP in offer")?;

        log::info!("Sending offer to server");

        let (client_id, _parent_id_from_server, _ip_version_from_server, answer_sdp, server_conn_id) =
            signaling::send_offer_with_mode(offer_sdp.clone(), parent_client_id.clone(), Some(ip_version.to_string()), mode, Some(generated_conn_id.clone())).await?;

        log::info!("Received answer from server, client_id: {}, conn_id: {}", client_id, server_conn_id);
        
        // Update state with conn_id from server
        state.borrow_mut().conn_id = server_conn_id.clone();

        // Set up ICE candidate event handler BEFORE setLocalDescription
        // This is critical - ICE gathering starts as soon as we set local description
        let _peer_clone = peer.clone();
        let client_id_clone = client_id.clone();
        let ip_version_for_filter = ip_version.to_string();
        let onicecandidate = Closure::wrap(Box::new(move |event: web_sys::Event| {
            // Check if this is an icecandidate event
            if event.type_() == "icecandidate" {
                if let Ok(candidate) = js_sys::Reflect::get(&event, &"candidate".into()) {
                    if !candidate.is_undefined() && !candidate.is_null() {
                        // Extract the candidate SDP string for filtering
                        let candidate_sdp = js_sys::Reflect::get(&candidate, &"candidate".into())
                            .ok()
                            .and_then(|c| c.as_string());

                        // Convert JsValue to JSON string
                        if let Ok(json_str) = js_sys::JSON::stringify(&candidate) {
                            let candidate_str = json_str.as_string().unwrap_or_default();

                            // Skip empty candidates (end-of-candidates)
                            if !candidate_str.trim().is_empty() && !candidate_str.contains("\"\"") {
                                // Filter candidates by IP version
                                let should_send = if let Some(sdp) = candidate_sdp {
                                    if let Some(detected_version) = get_candidate_ip_version(&sdp) {
                                        let matches = detected_version.eq_ignore_ascii_case(&ip_version_for_filter);
                                        if matches {
                                            log::info!("Sending {} candidate: {}", detected_version, sdp);
                                        } else {
                                            log::debug!("Filtering out {} candidate for {} connection: {}",
                                                detected_version, ip_version_for_filter, sdp);
                                        }
                                        matches
                                    } else {
                                        // Unable to determine version, send it anyway (might be relay/reflexive)
                                        log::debug!("Unable to determine IP version, sending candidate anyway: {:?}", sdp);
                                        true
                                    }
                                } else {
                                    // No SDP string, send anyway
                                    true
                                };

                                if should_send {
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
            }
        }) as Box<dyn FnMut(_)>);

        peer.set_onicecandidate(Some(onicecandidate.as_ref().unchecked_ref()));
        onicecandidate.forget();

        // NOW set local and remote descriptions (ICE gathering will start after setLocalDescription)
        let offer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        offer_obj.set_sdp(&offer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_local_description(&offer_obj)
        ).await?;

        let answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        answer_obj.set_sdp(&answer_sdp);
        log::info!("Remote answer: {:?}", &answer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_remote_description(&answer_obj)
        ).await?;

        // Start polling for server ICE candidates
        let peer_for_poll = peer.clone();
        let client_id_for_poll = client_id.clone();
        let ip_version_for_poll = ip_version.to_string();
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
                                if let Some(candidate_sdp) = candidate.as_string() {
                                    // Filter server candidates by IP version
                                    let should_add = if let Some(detected_version) = get_candidate_ip_version(&candidate_sdp) {
                                        let matches = detected_version.eq_ignore_ascii_case(&ip_version_for_poll);
                                        if matches {
                                            log::info!("Adding {} candidate from server: {}", detected_version, candidate_sdp);
                                        } else {
                                            log::debug!("Filtering out server {} candidate for {} connection: {}",
                                                detected_version, ip_version_for_poll, candidate_sdp);
                                        }
                                        matches
                                    } else {
                                        // Unable to determine version, add it anyway
                                        log::debug!("Unable to determine server candidate IP version, adding anyway: {}", candidate_sdp);
                                        true
                                    };

                                    if should_add {
                                        let candidate_init = RtcIceCandidateInit::new(&candidate_sdp);
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
                }

                if should_stop {
                    log::info!("Stopping ICE candidate polling");
                    break;
                }
            }
        });

        log::info!("WebRTC connection established");

        Ok(Self { peer, client_id, conn_id: server_conn_id, state, control_channel: control_channel_ref })
    }
    
    /// Send a stop traceroute message to the server
    pub async fn send_stop_traceroute(&self) -> Result<(), JsValue> {
        let stop_msg = common::StopTracerouteMessage {
            conn_id: self.conn_id.clone(),
        };
        
        let json = serde_json::to_vec(&stop_msg)
            .map_err(|e| {
                log::error!("Failed to serialize stop traceroute message: {}", e);
                JsValue::from_str(&format!("Serialization error: {}", e))
            })?;
        
        let control_channel_opt = self.control_channel.borrow();
        if let Some(channel) = control_channel_opt.as_ref() {
            // Convert Vec<u8> to js_sys::Uint8Array and send
            let array = js_sys::Uint8Array::from(&json[..]);
            
            // Send the message using ArrayBuffer
            channel.send_with_array_buffer(&array.buffer())?;
            log::info!("Sent stop traceroute message for conn_id: {}", self.conn_id);
        } else {
            log::warn!("Control channel not available to send stop traceroute message");
        }
        
        Ok(())
    }
}
