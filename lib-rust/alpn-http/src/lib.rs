use anyhow::Result;
use rpc::ServiceRpc;
use alpn_base::ProtocolHandler;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct HttpHandler;

impl HttpHandler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProtocolHandler for HttpHandler {
    fn protocol_id(&self) -> String {
        "http".to_string()
    }

    async fn setup(&self, services: HashMap<String, ServiceRpc>) -> Result<()> {
        println!("HTTP Handler setting up services: {:?}", services.keys());
        Ok(())
    }
}
