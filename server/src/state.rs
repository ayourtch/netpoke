use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use webrtc::peer_connection::RTCPeerConnection;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, Arc<ClientSession>>>>,
}

pub struct ClientSession {
    pub id: String,
    pub peer_connection: Arc<RTCPeerConnection>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
