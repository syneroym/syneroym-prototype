use anyhow::Result;
use common::config::Config;
use tracing::info;

pub struct PeerNode {
    config: Config,
}

impl PeerNode {
    pub async fn new(config: Config) -> Result<Self> {
        Ok(Self { config })
    }

    pub async fn bootstrap(&self) -> Result<()> {
        info!("Bootstrapping Syneroym PeerNode...");
        
        // Initialize Networking
        self.init_networking().await?;

        // Future: Initialize Storage, etc.
        
        info!("PeerNode bootstrapped successfully.");
        Ok(())
    }

    async fn init_networking(&self) -> Result<()> {
        for comm in &self.config.enabled_comms {
            match comm.as_str() {
                "iroh" => {
                    info!("Initializing Iroh interface...");
                    net_iroh::init(&self.config).await?;
                }
                _ => {
                    info!("Unknown or unimplemented communication interface: {}", comm);
                }
            }
        }
        Ok(())
    }
}
