use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize, Serialize)]
pub struct Config {
    /// Iroh communication configuration
    pub comm_iroh: Option<IrohCommConfig>,
    /// WebRTC communication configuration
    pub comm_webrtc: Option<WebRtcCommConfig>,
    /// List of enabled communication interfaces (e.g. "iroh", "webrtc")
    pub enabled_comms: Vec<String>,
    /// List of ALPN protocols to accept/handle.
    pub alpn_protocols: Vec<String>,
    /// Path to the local data store (rqlite/sqlite).
    pub data_store_path: PathBuf,
    /// Peer Gateway configuration
    pub peer_gateway: Option<PeerGatewayConfig>,
    /// Signaling Server configuration
    pub signaling_server: Option<SignalingServerConfig>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            comm_iroh: Some(IrohCommConfig::default()),
            comm_webrtc: Some(WebRtcCommConfig::default()),
            enabled_comms: vec!["iroh".to_string()],
            alpn_protocols: vec![],
            data_store_path: PathBuf::from("syneroym_data.db"),
            peer_gateway: Some(PeerGatewayConfig::default()),
            signaling_server: Some(SignalingServerConfig::default()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct PeerGatewayConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for PeerGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8001,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SignalingServerConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for SignalingServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8000,
        }
    }
}

#[derive(Deserialize, Serialize, Default)]
pub struct IrohCommConfig {
    /// Path to the secret key file for the Iroh node identity.
    /// If not provided, a temporary identity may be generated or a default location used.
    pub secret_key_path: Option<PathBuf>,
    /// Optional custom Relay URL to use. If None, the default relay map is used.
    pub relay_url: Option<String>,
    /// Optional port to bind the Iroh RPC to.
    pub rpc_port: Option<u16>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct WebRtcCommConfig {
    /// URL of the signaling server
    pub signaling_server_url: Option<String>,
}
