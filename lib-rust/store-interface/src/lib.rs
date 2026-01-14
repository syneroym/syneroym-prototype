use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceRecord {
    pub service_key: String,
    pub app_layer_protocol: String,
    pub service_image_manifest_ref: String,
}

#[async_trait]
pub trait ServiceStore: Send + Sync {
    /// Retrieve all configured services.
    async fn get_services(&self) -> Result<Vec<ServiceRecord>>;
}
