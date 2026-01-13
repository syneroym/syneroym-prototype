use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
pub struct Config {
    /// Iroh communication configuration
    pub comm_iroh: Option<IrohCommConfig>,
    /// List of enabled communication interfaces (e.g. "iroh", "webrtc")
    pub enabled_comms: Vec<String>,
    /// List of ALPN protocols to accept/handle.
    pub alpn_protocols: Vec<String>,
    /// Path to the local data store (rqlite/sqlite).
    pub data_store_path: PathBuf,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            comm_iroh: Some(IrohCommConfig::default()),
            enabled_comms: vec!["iroh".to_string()],
            alpn_protocols: vec![],
            data_store_path: PathBuf::from("syneroym_data.db"),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct IrohCommConfig {
    /// Path to the secret key file for the Iroh node identity.
    /// If not provided, a temporary identity may be generated or a default location used.
    pub secret_key_path: Option<PathBuf>,
    /// Optional custom Relay URL to use. If None, the default relay map is used.
    pub relay_url: Option<String>,
    /// Optional port to bind the Iroh RPC to.
    pub rpc_port: Option<u16>,
}

impl Default for IrohCommConfig {
    fn default() -> Self {
        Self {
            secret_key_path: None,
            relay_url: None,
            rpc_port: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub service_key: String,
    pub app_layer_protocol: String,
    pub service_image_manifest_ref: String,
}