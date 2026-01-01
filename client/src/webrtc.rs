use wasm_bindgen::prelude::*;
use web_sys::{
    RtcPeerConnection, RtcConfiguration,
    RtcDataChannelInit, RtcSessionDescriptionInit, RtcSdpType,
    RtcIceCandidateInit, RtcDataChannelState,
};
use js_sys;
use crate::{signaling, measurements};
use std::rc::Rc;
use std::cell::RefCell;
use common::{is_name_based_candidate, get_candidate_ip_version};

/// Length to truncate client IDs for logging (first 8 characters)
const CLIENT_ID_LOG_LENGTH: usize = 8;

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

/// Update peer connection addresses in the UI by fetching stats from the RTCPeerConnection
async fn update_peer_connection_addresses_from_stats(peer: &RtcPeerConnection, ip_version: &str, conn_index: usize) -> Result<(), JsValue> {
    // Get stats from the peer connection
    let stats_promise = peer.get_stats();
    let stats_result = wasm_bindgen_futures::JsFuture::from(stats_promise).await?;
    
    // The result is an RTCStatsReport which is a Map-like object
    let stats_report = stats_result;
    
    // Find the selected candidate pair
    let mut local_address = None;
    let mut remote_address = None;
    let mut local_candidate_id = None;
    let mut remote_candidate_id = None;
    
    // Iterate through the stats using the values() iterator
    // RTCStatsReport is a Map-like object with a values() method that returns an iterator
    let values_fn = js_sys::Reflect::get(&stats_report, &"values".into())?;
    if let Some(values) = values_fn.dyn_ref::<js_sys::Function>() {
        let iterator = values.call0(&stats_report)?;
        
        // Collect all stats into a vector first
        let mut all_stats = Vec::new();
        loop {
            let next_fn = js_sys::Reflect::get(&iterator, &"next".into())?;
            if let Some(next) = next_fn.dyn_ref::<js_sys::Function>() {
                let result = next.call0(&iterator)?;
                let done = js_sys::Reflect::get(&result, &"done".into())?;
                if done.as_bool().unwrap_or(true) {
                    break;
                }
                let value = js_sys::Reflect::get(&result, &"value".into())?;
                all_stats.push(value);
            } else {
                break;
            }
        }
        
        // Find selected candidate pair
        for stat in &all_stats {
            let stat_type = js_sys::Reflect::get(stat, &"type".into())
                .ok()
                .and_then(|t| t.as_string());
            
            if stat_type.as_deref() == Some("candidate-pair") {
                // Check if this is the selected/nominated pair
                let selected = js_sys::Reflect::get(stat, &"selected".into())
                    .ok()
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);
                let nominated = js_sys::Reflect::get(stat, &"nominated".into())
                    .ok()
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);
                let state = js_sys::Reflect::get(stat, &"state".into())
                    .ok()
                    .and_then(|s| s.as_string());
                
                // A candidate pair is active if it's selected/nominated and in succeeded state
                if (selected || nominated) && state.as_deref() == Some("succeeded") {
                    local_candidate_id = js_sys::Reflect::get(stat, &"localCandidateId".into())
                        .ok()
                        .and_then(|s| s.as_string());
                    remote_candidate_id = js_sys::Reflect::get(stat, &"remoteCandidateId".into())
                        .ok()
                        .and_then(|s| s.as_string());
                    break;
                }
            }
        }
        
        // Now find the actual candidate addresses
        if let (Some(local_id), Some(remote_id)) = (&local_candidate_id, &remote_candidate_id) {
            for stat in &all_stats {
                let stat_id = js_sys::Reflect::get(stat, &"id".into())
                    .ok()
                    .and_then(|s| s.as_string());
                let stat_type = js_sys::Reflect::get(stat, &"type".into())
                    .ok()
                    .and_then(|t| t.as_string());
                
                if stat_type.as_deref() == Some("local-candidate") || stat_type.as_deref() == Some("remote-candidate") {
                    if let Some(ref id) = stat_id {
                        let address = js_sys::Reflect::get(stat, &"address".into())
                            .ok()
                            .and_then(|a| a.as_string());
                        let port = js_sys::Reflect::get(stat, &"port".into())
                            .ok()
                            .and_then(|p| p.as_f64())
                            .map(|p| p as u16);
                        
                        if id == local_id {
                            if let (Some(addr), Some(p)) = (address, port) {
                                local_address = Some(format!("{}:{}", addr, p));
                            }
                        } else if id == remote_id {
                            if let (Some(addr), Some(p)) = (address, port) {
                                remote_address = Some(format!("{}:{}", addr, p));
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Update the UI if we found addresses
    if local_address.is_some() || remote_address.is_some() {
        update_peer_connection_addresses_js(
            ip_version,
            conn_index,
            local_address.as_deref(),
            remote_address.as_deref()
        );
        
        log::info!("Updated {} connection {} addresses: local={:?}, remote={:?}",
            ip_version, conn_index, local_address, remote_address);
    }
    
    Ok(())
}

/// Call JavaScript function to update peer connection addresses
fn update_peer_connection_addresses_js(ip_version: &str, conn_index: usize, local_address: Option<&str>, remote_address: Option<&str>) {
    use wasm_bindgen::JsCast;
    
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    
    // Call JavaScript function updatePeerConnectionAddresses(ipVersion, connIndex, localAddress, remoteAddress)
    if let Ok(update_fn) = js_sys::Reflect::get(&window, &JsValue::from_str("updatePeerConnectionAddresses")) {
        if let Some(func) = update_fn.dyn_ref::<js_sys::Function>() {
            let args = js_sys::Array::new();
            args.push(&JsValue::from_str(ip_version));
            args.push(&JsValue::from_f64(conn_index as f64));
            args.push(&local_address.map(JsValue::from_str).unwrap_or(JsValue::NULL));
            args.push(&remote_address.map(JsValue::from_str).unwrap_or(JsValue::NULL));
            
            if let Err(e) = func.apply(&JsValue::NULL, &args) {
                log::warn!("Failed to call updatePeerConnectionAddresses: {:?}", e);
            }
        }
    }
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

        // Create bulk channel (unreliable, unordered) for realistic throughput measurement
        let bulk_init = RtcDataChannelInit::new();
        bulk_init.set_ordered(false);
        bulk_init.set_max_retransmits(0);
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
                                // Filter candidates by IP version and exclude name-based (mDNS) candidates
                                let should_send = if let Some(ref sdp) = candidate_sdp {
                                    // First, filter out name-based candidates (e.g., xxx.local mDNS)
                                    if is_name_based_candidate(sdp) {
                                        log::debug!("Filtering out name-based (mDNS) candidate: {}", sdp);
                                        false
                                    } else if let Some(detected_version) = get_candidate_ip_version(sdp) {
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

        // Set up ICE connection state change handler for debugging and monitoring
        let client_id_for_ice_state = client_id.clone();
        let ip_version_for_ice_state = ip_version.to_string();
        let peer_for_ice_state = peer.clone();
        let oniceconnectionstatechange = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            // Get the ice connection state from the peer
            let ice_state = js_sys::Reflect::get(&peer_for_ice_state, &"iceConnectionState".into())
                .ok()
                .and_then(|s| s.as_string())
                .unwrap_or_else(|| "unknown".to_string());
            log::info!("[{}][{}] ICE connection state: {}", 
                ip_version_for_ice_state, 
                &client_id_for_ice_state[..CLIENT_ID_LOG_LENGTH.min(client_id_for_ice_state.len())],
                ice_state);
        }) as Box<dyn FnMut(_)>);
        peer.set_oniceconnectionstatechange(Some(oniceconnectionstatechange.as_ref().unchecked_ref()));
        oniceconnectionstatechange.forget();

        // Set up ICE gathering state change handler for debugging
        let client_id_for_gathering = client_id.clone();
        let ip_version_for_gathering = ip_version.to_string();
        let peer_for_gathering = peer.clone();
        let onicegatheringstatechange = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            let gathering_state = js_sys::Reflect::get(&peer_for_gathering, &"iceGatheringState".into())
                .ok()
                .and_then(|s| s.as_string())
                .unwrap_or_else(|| "unknown".to_string());
            log::info!("[{}][{}] ICE gathering state: {}", 
                ip_version_for_gathering,
                &client_id_for_gathering[..CLIENT_ID_LOG_LENGTH.min(client_id_for_gathering.len())],
                gathering_state);
        }) as Box<dyn FnMut(_)>);
        peer.set_onicegatheringstatechange(Some(onicegatheringstatechange.as_ref().unchecked_ref()));
        onicegatheringstatechange.forget();

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
                                    // Filter server candidates by IP version and exclude name-based (mDNS) candidates
                                    let should_add = if is_name_based_candidate(&candidate_sdp) {
                                        // Filter out name-based candidates (e.g., xxx.local mDNS)
                                        log::debug!("Filtering out name-based (mDNS) server candidate: {}", candidate_sdp);
                                        false
                                    } else if let Some(detected_version) = get_candidate_ip_version(&candidate_sdp) {
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
    
    /// Helper method to send a serializable message over the control channel
    /// Returns an error if the control channel is not ready (not open)
    fn send_control_message<T: serde::Serialize>(&self, msg: &T, msg_type: &str) -> Result<(), JsValue> {
        let json = serde_json::to_string(msg)
            .map_err(|e| {
                log::error!("Failed to serialize {} message: {}", msg_type, e);
                JsValue::from_str(&format!("Serialization error: {}", e))
            })?;
        
        let control_channel_opt = self.control_channel.borrow();
        if let Some(channel) = control_channel_opt.as_ref() {
            // Check if channel is open before sending
            let state = channel.ready_state();
            if state != RtcDataChannelState::Open {
                log::warn!("Control channel not open for {} message (state: {:?})", msg_type, state);
                return Err(JsValue::from_str(&format!(
                    "Control channel not ready: {:?}", state
                )));
            }
            
            channel.send_with_str(&json)?;
            log::info!("Sent {} message for conn_id: {}", msg_type, self.conn_id);
        } else {
            log::warn!("Control channel not available to send {} message", msg_type);
            return Err(JsValue::from_str("Control channel not available"));
        }
        
        Ok(())
    }
    
    /// Check if the control channel is open and ready for sending messages
    pub fn is_control_channel_open(&self) -> bool {
        let control_channel_opt = self.control_channel.borrow();
        if let Some(channel) = control_channel_opt.as_ref() {
            channel.ready_state() == RtcDataChannelState::Open
        } else {
            false
        }
    }
    
    /// Wait for the control channel to be ready with a timeout
    /// Returns true if the channel is ready, false if timeout occurred
    pub async fn wait_for_control_channel_ready(&self, timeout_ms: u32) -> bool {
        let start_time = crate::measurements::current_time_ms();
        let timeout = timeout_ms as u64;
        
        loop {
            if self.is_control_channel_open() {
                log::info!("Control channel is now open for conn_id: {}", self.conn_id);
                return true;
            }
            
            let elapsed = crate::measurements::current_time_ms() - start_time;
            if elapsed >= timeout {
                log::warn!("Timeout waiting for control channel to open for conn_id: {} ({}ms)", 
                    self.conn_id, timeout_ms);
                return false;
            }
            
            // Sleep for 50ms before checking again
            crate::sleep_ms(50).await;
        }
    }
    
    /// Send a stop traceroute message to the server
    pub async fn send_stop_traceroute(&self, survey_session_id: &str) -> Result<(), JsValue> {
        let msg = common::StopTracerouteMessage {
            conn_id: self.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
        };
        self.send_control_message(&msg, "stop traceroute")?;
        self.state.borrow_mut().set_traceroute_active(false);
        Ok(())
    }

    /// Send a start traceroute message to the server
    pub async fn send_start_traceroute(&self, survey_session_id: &str) -> Result<(), JsValue> {
        let msg = common::StartTracerouteMessage {
            conn_id: self.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
        };
        self.send_control_message(&msg, "start traceroute")
    }
    
    /// Send a start survey session message to the server
    pub async fn send_start_survey_session(&self, survey_session_id: &str) -> Result<(), JsValue> {
        let msg = common::StartSurveySessionMessage {
            survey_session_id: survey_session_id.to_string(),
            conn_id: self.conn_id.clone(),
        };
        self.send_control_message(&msg, "start survey session")
    }
    
    /// Send a start MTU traceroute message to the server
    pub async fn send_start_mtu_traceroute(&self, survey_session_id: &str, packet_size: u32) -> Result<(), JsValue> {
        let msg = common::StartMtuTracerouteMessage {
            conn_id: self.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
            packet_size,
        };
        self.send_control_message(&msg, &format!("start MTU traceroute (size: {})", packet_size))
    }
    
    /// Send get measuring time message to the server
    pub async fn send_get_measuring_time(&self, survey_session_id: &str) -> Result<(), JsValue> {
        let msg = common::GetMeasuringTimeMessage {
            conn_id: self.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
        };
        self.send_control_message(&msg, "get measuring time")
    }
    
    /// Send start server traffic message to the server
    pub async fn send_start_server_traffic(&self, survey_session_id: &str) -> Result<(), JsValue> {
        let msg = common::StartServerTrafficMessage {
            conn_id: self.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
        };
        self.send_control_message(&msg, "start server traffic")?;
        self.state.borrow_mut().set_traceroute_active(false);
        Ok(())
    }
    
    /// Send stop server traffic message to the server
    pub async fn send_stop_server_traffic(&self, survey_session_id: &str) -> Result<(), JsValue> {
        let msg = common::StopServerTrafficMessage {
            conn_id: self.conn_id.clone(),
            survey_session_id: survey_session_id.to_string(),
        };
        self.send_control_message(&msg, "stop server traffic")
    }
    
    /// Enable traceroute mode (prevents measurement data collection)
    pub fn set_traceroute_mode(&self, active: bool) {
        self.state.borrow_mut().set_traceroute_active(active);
    }
    
    /// Set up a callback to update the peer connection addresses when the connection state changes
    /// 
    /// This should be called after creating the connection, providing the IP version and connection
    /// index so the UI can be updated with the actual local/remote addresses when the connection
    /// is established.
    /// 
    /// Note: We listen to both `connectionstatechange` and `iceconnectionstatechange` events for
    /// Safari compatibility. Safari doesn't consistently support `connectionState` property or
    /// fire `connectionstatechange` events, but `iceConnectionState` is well-supported.
    pub fn setup_address_update_callback(&self, ip_version: &str, conn_index: usize) {
        // Shared flag to prevent duplicate address updates from multiple event listeners.
        // We listen to both connectionstatechange and iceconnectionstatechange for Safari compatibility,
        // and this flag ensures only the first successful connection trigger updates the UI.
        let addresses_updated = Rc::new(RefCell::new(false));
        
        let peer = self.peer.clone();
        let ip_version_owned = ip_version.to_string();
        let addresses_updated_for_conn = addresses_updated.clone();
        
        let onconnectionstatechange = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            // Check if already updated
            if *addresses_updated_for_conn.borrow() {
                return;
            }
            
            let connection_state = js_sys::Reflect::get(&peer, &"connectionState".into())
                .ok()
                .and_then(|s| s.as_string());
            
            if let Some(state) = connection_state {
                log::info!("Connection state changed to: {}", state);
                
                if state == "connected" {
                    // Mark as updated before spawning to prevent race conditions
                    *addresses_updated_for_conn.borrow_mut() = true;
                    
                    // Connection is established, fetch the selected candidate pair addresses
                    let peer_clone = peer.clone();
                    let ip_version_clone = ip_version_owned.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Err(e) = update_peer_connection_addresses_from_stats(&peer_clone, &ip_version_clone, conn_index).await {
                            log::warn!("Failed to update peer connection addresses: {:?}", e);
                        }
                    });
                }
            }
        }) as Box<dyn FnMut(_)>);

        self.peer.set_onconnectionstatechange(Some(onconnectionstatechange.as_ref().unchecked_ref()));
        // Note: forget() is required to prevent the closure from being dropped when this function returns.
        // The closure will live as long as the peer connection, and will be cleaned up when the peer is closed.
        // This is the standard pattern for WebRTC callbacks in WASM.
        onconnectionstatechange.forget();
        
        // Also listen for ICE connection state changes for Safari compatibility.
        // Safari doesn't consistently support connectionState, but iceConnectionState is well-supported.
        let peer_for_ice = self.peer.clone();
        let ip_version_for_ice = ip_version.to_string();
        let addresses_updated_for_ice = addresses_updated.clone();
        
        let oniceconnectionstatechange_for_address = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            // Check if already updated
            if *addresses_updated_for_ice.borrow() {
                return;
            }
            
            let ice_state = js_sys::Reflect::get(&peer_for_ice, &"iceConnectionState".into())
                .ok()
                .and_then(|s| s.as_string());
            
            if let Some(state) = ice_state {
                log::info!("ICE connection state changed to: {}", state);
                
                // "connected" or "completed" indicates a successful ICE connection
                if state == "connected" || state == "completed" {
                    // Mark as updated before spawning to prevent race conditions
                    *addresses_updated_for_ice.borrow_mut() = true;
                    
                    // Connection is established, fetch the selected candidate pair addresses
                    let peer_clone = peer_for_ice.clone();
                    let ip_version_clone = ip_version_for_ice.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Err(e) = update_peer_connection_addresses_from_stats(&peer_clone, &ip_version_clone, conn_index).await {
                            log::warn!("Failed to update peer connection addresses from ICE state: {:?}", e);
                        }
                    });
                }
            }
        }) as Box<dyn FnMut(_)>);

        // Add iceconnectionstatechange handler for address updates.
        // Note: This is separate from the debug handler in new_with_ip_version_and_mode which logs
        // ICE state for all connections. This handler specifically triggers address updates and
        // uses the shared flag to coordinate with connectionstatechange. Using addEventListener
        // allows both handlers to coexist.
        use wasm_bindgen::JsCast;
        let _ = self.peer.add_event_listener_with_callback(
            "iceconnectionstatechange",
            oniceconnectionstatechange_for_address.as_ref().unchecked_ref()
        );
        oniceconnectionstatechange_for_address.forget();
        
        log::info!("Set up address update callback for {} connection {} (with Safari compatibility)", ip_version, conn_index);
    }
}
