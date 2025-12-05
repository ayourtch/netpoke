use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<RwLock<HashMap<String, ClientSession>>>,
}

pub struct ClientSession {
    pub id: String,
    pub pending_ice_candidates: Vec<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ClientSession {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pending_ice_candidates: Vec::new(),
        }
    }
}
