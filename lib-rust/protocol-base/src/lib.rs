use anyhow::Result;
use app_host::ServiceRpc;
use async_trait::async_trait;
use std::collections::HashMap;

pub const SYNEROYM_ALPN: &[u8] = b"syneroym/1.0";

#[async_trait]
pub trait ProtocolHandler: Send + Sync {
    /// Returns the protocol identifier (e.g., "http")
    fn protocol_id(&self) -> String;

    /// Setup the handler with the necessary services and their RPC interfaces
    async fn setup(&self, services: HashMap<String, ServiceRpc>) -> Result<()>;
}
