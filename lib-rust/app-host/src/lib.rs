use store_interface::ServiceRecord;

#[derive(Debug, Clone)]
pub struct ServiceRpc {
    pub config: ServiceRecord,
    // Add RPC client/connection details here
}

impl ServiceRpc {
    pub fn new(config: ServiceRecord) -> Self {
        Self { config }
    }
}
