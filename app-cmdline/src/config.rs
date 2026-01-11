use serde::{Deserialize, Serialize};

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
            iroh_comm: Some(IrohCommConfig {}),
            http_txp: true,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct IrohCommConfig {}
