use common::config::Config;
use anyhow::Result;

pub async fn init(config: &Config) -> Result<()> {
    println!("Initializing networking layer...");
    
    // Initialize Iroh if configured
    if let Some(iroh_config) = &config.comm_iroh {
        println!("Iroh communication enabled.");
        if let Some(secret) = &iroh_config.secret_key_path {
             println!("Using secret key at: {:?}", secret);
        }
    }

    Ok(())
}
