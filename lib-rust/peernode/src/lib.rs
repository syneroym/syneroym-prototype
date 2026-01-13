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
        net::init(&self.config).await?;

        // Future: Initialize Storage, etc.
        
        info!("PeerNode bootstrapped successfully.");
        Ok(())
    }
}
