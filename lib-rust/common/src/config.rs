use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
pub struct Config {
    /// Iroh communication configuration
    pub iroh_comm: Option<IrohCommConfig>,
    /// Enable http transport support
    pub http_txp: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            iroh_comm: Some(IrohCommConfig::default()),
            http_txp: false,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct IrohCommConfig {
    /// Path to the secret key file for the Iroh node identity.
    /// If not provided, a temporary identity may be generated or a default location used.
    pub secret_key_path: Option<PathBuf>,
    /// List of ALPN protocols to accept/handle.
    pub alpn_protocols: Vec<String>,
    /// Optional custom Relay URL to use. If None, the default relay map is used.
    pub relay_url: Option<String>,
    /// Optional port to bind the Iroh RPC to.
    pub rpc_port: Option<u16>,
}

impl Default for IrohCommConfig {
    fn default() -> Self {
        Self {
            secret_key_path: None,
            alpn_protocols: vec![],
            relay_url: None,
            rpc_port: None,
        }
    }
}
