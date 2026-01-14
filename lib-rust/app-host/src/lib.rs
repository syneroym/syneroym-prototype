use common::config::ServiceConfig;

#[derive(Debug, Clone)]
pub struct ServiceRpc {
    pub config: ServiceConfig,
    // Add RPC client/connection details here
}

impl ServiceRpc {
    pub fn new(config: ServiceConfig) -> Self {
        Self { config }
    }
}