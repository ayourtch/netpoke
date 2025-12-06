use wasm_bindgen::prelude::*;
use web_sys::{
    RtcPeerConnection, RtcConfiguration,
    RtcDataChannelInit, RtcSessionDescriptionInit, RtcSdpType,
};
use crate::signaling;

pub struct WebRtcConnection {
    pub peer: RtcPeerConnection,
    pub client_id: String,
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

        log::info!("Creating data channels");

        // Create probe channel (unreliable, unordered)
        let mut probe_init = RtcDataChannelInit::new();
        probe_init.set_ordered(false);
        probe_init.set_max_retransmits(0);
        let _probe_channel = peer.create_data_channel_with_data_channel_dict("probe", &probe_init);

        // Create bulk channel (reliable, ordered)
        let bulk_init = RtcDataChannelInit::new();
        let _bulk_channel = peer.create_data_channel_with_data_channel_dict("bulk", &bulk_init);

        // Create control channel (reliable, ordered)
        let control_init = RtcDataChannelInit::new();
        let _control_channel = peer.create_data_channel_with_data_channel_dict("control", &control_init);

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

        Ok(Self { peer, client_id })
    }
}
