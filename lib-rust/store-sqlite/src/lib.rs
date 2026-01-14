use anyhow::Result;
use async_trait::async_trait;
use common::config::ServiceConfig;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use store_interface::ServiceStore;
use tracing::info;

pub struct SqliteStore {
    // Arc<Mutex<>> is needed because rusqlite::Connection is not Sync
    // In a real high-perf app, we'd use a connection pool (like r2d2) or tokio-rusqlite
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn new(path: PathBuf) -> Result<Self> {
        info!("Opening SQLite store at {:?}", path);
        let conn = Connection::open(path)?;
        
        // Ensure table exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS services (
                service_key TEXT PRIMARY KEY,
                app_layer_protocol TEXT NOT NULL,
                service_image_manifest_ref TEXT NOT NULL
            )",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl ServiceStore for SqliteStore {
    async fn get_services(&self) -> Result<Vec<ServiceConfig>> {
        let conn = self.conn.lock().unwrap();
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
}
