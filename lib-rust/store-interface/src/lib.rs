use anyhow::Result;
use async_trait::async_trait;
use common::config::ServiceConfig;

#[async_trait]
pub trait ServiceStore: Send + Sync {
    /// Retrieve all configured services.
    async fn get_services(&self) -> Result<Vec<ServiceConfig>>;
}
