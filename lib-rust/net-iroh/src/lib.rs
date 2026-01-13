use common::config::Config;
use alpn_base::ProtocolHandler;
use anyhow::Result;
use std::sync::Arc;

pub async fn init(config: &Config, handlers: Vec<Arc<dyn ProtocolHandler>>) -> Result<()> {
    // Initialize Iroh if configured
    if let Some(iroh_config) = &config.comm_iroh {
        println!("Initializing Iroh communication...");
        if let Some(secret) = &iroh_config.secret_key_path {
             println!("Using secret key at: {:?}", secret);
        }
        
        println!("Iroh passing connections to handlers:");
        for handler in handlers {
            println!(" - {}", handler.protocol_id());
        }
    }
    Ok(())
}