use alpn_base::ProtocolHandler;
use anyhow::Result;
use common::config::{Config, ServiceConfig};
use rpc::ServiceRpc;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

pub struct PeerNode {
    config: Config,
}

impl PeerNode {
    pub async fn new(config: Config) -> Result<Self> {
        Ok(Self { config })
    }

    pub async fn bootstrap(&self) -> Result<()> {
        info!("Bootstrapping Syneroym PeerNode...");

        // 1. Read Services Configuration
        let services = self.fetch_services()?;

        // 2. Initialize Service RPC
        let service_rpcs = self.init_service_rpc(&services).await?;

        // 3. Initialize Protocol Handlers
        let handlers = self.init_protocol_handlers(&services, service_rpcs).await?;

        // 4. Initialize Networking
        self.init_networking(handlers).await?;

        info!("PeerNode bootstrapped successfully.");
        Ok(())
    }

    async fn init_networking(&self, handlers: Vec<Arc<dyn ProtocolHandler>>) -> Result<()> {
        for comm in &self.config.enabled_comms {
            match comm.as_str() {
                "iroh" => {
                    info!("Initializing Iroh interface...");
                    net_iroh::init(&self.config, handlers.clone()).await?;
                },
                _ => {
                    info!("Unknown or unimplemented communication interface: {}", comm);
                },
            }
        }
        Ok(())
    }

    fn fetch_services(&self) -> Result<Vec<ServiceConfig>> {
        info!(
            "Reading services from data store at {:?}...",
            self.config.data_store_path
        );
        let conn = Connection::open(&self.config.data_store_path)?;

        // Ensure table exists for this prototype
        conn.execute(
            "CREATE TABLE IF NOT EXISTS services (
                service_key TEXT PRIMARY KEY,
                app_layer_protocol TEXT NOT NULL,
                service_image_manifest_ref TEXT NOT NULL
            )",
            [],
        )?;

        let mut stmt = conn.prepare(
            "SELECT service_key, app_layer_protocol, service_image_manifest_ref FROM services",
        )?;
        let service_iter = stmt.query_map([], |row| {
            Ok(ServiceConfig {
                service_key: row.get(0)?,
                app_layer_protocol: row.get(1)?,
                service_image_manifest_ref: row.get(2)?,
            })
        })?;

        let mut services = Vec::new();
        for service in service_iter {
            services.push(service?);
        }
        Ok(services)
    }

    async fn init_service_rpc(
        &self,
        services: &[ServiceConfig],
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
        services: &[ServiceConfig],
        service_rpcs: HashMap<String, ServiceRpc>,
    ) -> Result<Vec<Arc<dyn ProtocolHandler>>> {
        let mut services_by_protocol: HashMap<String, Vec<ServiceConfig>> = HashMap::new();

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
                    let handler = Arc::new(alpn_http::HttpHandler::new());

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
