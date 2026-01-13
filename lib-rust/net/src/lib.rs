use common::config::Config;
use anyhow::Result;

pub async fn init(config: &Config) -> Result<()> {
    println!("Initializing networking layer...");
    
    if let Some(iroh_config) = &config.iroh_comm {
        println!("Iroh communication enabled.");
        println!("Secret Key Path: {:?}", iroh_config.secret_key_path);
        println!("ALPN Protocols: {:?}", iroh_config.alpn_protocols);
    } else {
        println!("Iroh communication disabled.");
    }

    if config.http_txp {
        println!("HTTP transport enabled.");
    }

    Ok(())
}
