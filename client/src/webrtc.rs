use wasm_bindgen::prelude::*;
use web_sys::{
    RtcPeerConnection, RtcConfiguration,
    RtcDataChannelInit, RtcSessionDescriptionInit, RtcSdpType,
};
use crate::{signaling, measurements};
use std::rc::Rc;
use std::cell::RefCell;

pub struct WebRtcConnection {
    pub peer: RtcPeerConnection,
    pub client_id: String,
    pub state: Rc<RefCell<measurements::MeasurementState>>,
}

impl WebRtcConnection {
    pub async fn new() -> Result<Self, JsValue> {
        log::info!("Creating RTCPeerConnection");

        let mut config = RtcConfiguration::new();
        // Use Google's public STUN server
        let ice_servers = js_sys::Array::new();
        let server = js_sys::Object::new();
        js_sys::Reflect::set(&server, &"urls".into(), &"stun:stun.l.google.com:19302".into())?;
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

        let (client_id, answer_sdp) = signaling::send_offer(offer_sdp).await?;

        log::info!("Received answer from server, client_id: {}", client_id);

        let mut answer_obj = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        answer_obj.set_sdp(&answer_sdp);

        wasm_bindgen_futures::JsFuture::from(
            peer.set_remote_description(&answer_obj)
        ).await?;

        log::info!("WebRTC connection established");

        Ok(Self { peer, client_id, state })
    }
}
