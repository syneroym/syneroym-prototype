use anyhow::Result;
use app_host::ServiceRpc;
use common::config::Config;
use protocol_base::ProtocolHandler;
use std::collections::HashMap;

use std::sync::Arc;
use store_interface::{ServiceRecord, ServiceStore};
use tracing::info;

pub struct LocalNode {
    config: Config,
    store: Arc<dyn ServiceStore>,
}

impl LocalNode {
    pub async fn new(config: Config) -> Result<Self> {
        // Initialize the store based on configuration (defaulting to SQLite for now)
        let store = Arc::new(store_sqlite::SqliteStore::new(
            config.data_store_path.clone(),
        )?);
        Ok(Self { config, store })
    }

    pub async fn bootstrap(&self) -> Result<()> {
        info!("Bootstrapping Syneroym LocalNode...");

        // 1. Read Services Configuration
        let services = self.fetch_services().await?;

        // 2. Initialize Service RPC
        let service_rpcs = self.init_service_rpc(&services).await?;

        // 3. Initialize Protocol Handlers
        let handlers = self.init_protocol_handlers(&services, service_rpcs).await?;

        // 4. Initialize Networking
        let router_opt = self.init_networking(handlers).await?;

        if let Some(router) = router_opt {
            let endpoint = router.endpoint();
            // wait for the endpoint to be online
            router.endpoint().online().await;

            let node_addr = endpoint.addr();
            info!("Starting LocalNode Proxy HTTP...");
            peer_proxy_http::start(3000, node_addr).await?;

            // This makes sure the endpoint in the router is closed properly and connections close gracefully
            router.shutdown().await?;
        }

        info!("LocalNode bootstrapped successfully.");
        Ok(())
    }

    async fn init_networking(
        &self,
        handlers: Vec<Arc<dyn ProtocolHandler>>,
    ) -> Result<Option<iroh::protocol::Router>> {
        let mut iroh_router = None;

        for comm in &self.config.enabled_comms {
            match comm.as_str() {
                "iroh" => {
                    info!("Initializing Iroh interface...");
                    if let Some(router) = net_iroh::init(&self.config, handlers.clone()).await? {
                        iroh_router = Some(router);
                    }
                },
                "webrtc" => {
                    info!("Initializing WebRTC interface...");
                    net_webrtc::init(&self.config, handlers.clone()).await?;
                },
                _ => {
                    info!("Unknown or unimplemented communication interface: {}", comm);
                },
            }
        }
        Ok(iroh_router)
    }

    async fn fetch_services(&self) -> Result<Vec<ServiceRecord>> {
        info!("Reading services from data store...",);
        let mut services = self.store.get_services().await?;

        // Add test services
        services.push(ServiceRecord {
            service_key: "test1".to_string(),
            app_layer_protocol: "http".to_string(),
            service_image_manifest_ref: "local-http/test1".to_string(),
        });
        services.push(ServiceRecord {
            service_key: "test2".to_string(),
            app_layer_protocol: "http".to_string(),
            service_image_manifest_ref: "local-http/test2".to_string(),
        });

        Ok(services)
    }

    async fn init_service_rpc(
        &self,
        services: &[ServiceRecord],
    ) -> Result<HashMap<String, ServiceRpc>> {
        info!(
            "Initializing Service RPC for {} services...",
            services.len()
        );
        let mut rpcs = HashMap::new();
        for service in services {
            // Placeholder: Create actual RPC connection/client here
            rpcs.insert(
                service.service_key.clone(),
                ServiceRpc::new(service.clone()),
            );
        }
        Ok(rpcs)
    }

    async fn init_protocol_handlers(
        &self,
        services: &[ServiceRecord],
        service_rpcs: HashMap<String, ServiceRpc>,
    ) -> Result<Vec<Arc<dyn ProtocolHandler>>> {
        let mut services_by_protocol: HashMap<String, Vec<ServiceRecord>> = HashMap::new();

        for service in services {
            services_by_protocol
                .entry(service.app_layer_protocol.clone())
                .or_default()
                .push(service.clone());
        }

        let mut handlers: Vec<Arc<dyn ProtocolHandler>> = Vec::new();

        for (protocol, protocol_services) in services_by_protocol {
            info!(
                "Initializing handler for protocol: {} ({} services)",
                protocol,
                protocol_services.len()
            );
            match protocol.as_str() {
                "http" => {
                    let handler = Arc::new(protocol_http::HttpHandler::new());

                    // Filter RPCs for this protocol's services
                    let mut protocol_rpcs = HashMap::new();
                    for s in protocol_services {
                        if let Some(rpc) = service_rpcs.get(&s.service_key) {
                            protocol_rpcs.insert(s.service_key.clone(), rpc.clone());
                        }
                    }

                    handler.setup(protocol_rpcs).await?;
                    handlers.push(handler);
                },
                _ => {
                    info!("No handler found for protocol: {}", protocol);
                },
            }
        }
        Ok(handlers)
    }
}
