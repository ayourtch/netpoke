use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::data_channel::RTCDataChannel;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
}

pub struct ClientSession {
    pub id: String,
    pub peer_connection: Arc<RTCPeerConnection>,
    pub data_channels: Arc<RwLock<DataChannels>>,
}

pub struct DataChannels {
    pub probe: Option<Arc<RTCDataChannel>>,
    pub bulk: Option<Arc<RTCDataChannel>>,
    pub control: Option<Arc<RTCDataChannel>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl DataChannels {
    pub fn new() -> Self {
        Self {
            probe: None,
            bulk: None,
            control: None,
        }
    }
}
