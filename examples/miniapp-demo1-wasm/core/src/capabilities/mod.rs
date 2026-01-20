pub mod db;
pub mod files;
pub mod messaging;
pub mod streams;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Host capabilities provided to WASM
#[derive(Clone)]
pub struct HostCapabilities {
    pub db: Arc<Mutex<Connection>>,
    pub data_dir: PathBuf,
    pub stream_manager: Arc<streams::StreamManager>,
    pub message_streams: Arc<messaging::MessageStreamManager>,
}

impl HostCapabilities {
    pub fn new(data_dir: &str, db_conn: Connection) -> Self {
        HostCapabilities {
            db: Arc::new(Mutex::new(db_conn)),
            data_dir: PathBuf::from(data_dir),
            stream_manager: Arc::new(streams::StreamManager::new()),
            message_streams: Arc::new(messaging::MessageStreamManager::new()),
        }
    }
}
