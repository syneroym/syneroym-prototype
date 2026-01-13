use anyhow::Result;
use async_trait::async_trait;
use rpc::ServiceRpc;
use std::collections::HashMap;

#[async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Returns the protocol identifier (e.g., "http")
    fn protocol_id(&self) -> String;

    /// Setup the handler with the necessary services and their RPC interfaces
    async fn setup(&self, services: HashMap<String, ServiceRpc>) -> Result<()>;
}
