use anyhow::Result;
use app_host::ServiceRpc;
use async_trait::async_trait;
use protocol_base::ProtocolHandler;
use std::collections::HashMap;

#[derive(Debug)]
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
