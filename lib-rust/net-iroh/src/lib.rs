use anyhow::Result;
use common::config::Config;
use common::stream::IrohStream;
use iroh::{
    endpoint::{Connection, RecvStream, SendStream},
    protocol::{AcceptError, ProtocolHandler as IrohProtocolHandler, Router},
    Endpoint,
};
use n0_error::e;
use n0_error::AnyError;
use protocol_base::{ProtocolHandler, SYNEROYM_ALPN};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tracing::{debug, error, info};

pub async fn init(
    config: &Config,
    handlers: Vec<Arc<dyn ProtocolHandler>>,
) -> Result<Option<Router>> {
    for handler in &handlers[..] {
        debug!(" - {}", handler.protocol_id());
    }

    // Initialize Iroh if configured
    if let Some(iroh_config) = &config.comm_iroh {
        debug!("Initializing Iroh communication...");
        if let Some(secret) = &iroh_config.secret_key_path {
            debug!("Using secret key at: {:?}", secret);
        }

        let router = start_accept_side(handlers).await?;

        info!(
            "Iroh listening on ALPN: {:?}",
            std::str::from_utf8(SYNEROYM_ALPN)
        );
        debug!("Iroh passing connections to handlers:");

        // Wait for the endpoint to be online (non-blocking if we just await the future, but we want to return it)
        // Actually, let's just return the router. The caller can wait if they want, or just hold it.
        // router.endpoint().online().await;

        return Ok(Some(router));
    }
    Ok(None)
}

// Taken from the Iroh echo example: https://github.com/n0-computer/iroh/blob/main/iroh/examples/echo.rs
async fn start_accept_side(handlers: Vec<Arc<dyn ProtocolHandler>>) -> Result<Router> {
    let endpoint = Endpoint::bind().await?;

    // Build our protocol handler and add our protocol, identified by its ALPN, and spawn the endpoint.
    let router = Router::builder(endpoint)
        .accept(SYNEROYM_ALPN, ServiceProxy { handlers })
        .spawn();

    Ok(router)
}

#[derive(Debug, Clone)]
struct ServiceProxy {
    #[allow(dead_code)]
    handlers: Vec<Arc<dyn ProtocolHandler>>,
}

impl IrohProtocolHandler for ServiceProxy {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        // We can get the remote's endpoint id from the connection.
        let endpoint_id = connection.remote_id();
        debug!("accepted connection from {endpoint_id}");

        // We expect the connecting peer to open a single bi-directional stream.
        let (send, recv) = connection.accept_bi().await?;

        let e = handle_stream((send, recv)).await;

        // Wait until the remote closes the connection, which it does once it
        // received the response.
        connection.closed().await;

        match e {
            Ok(_) => Ok(()),
            Err(_) => e,
        }
    }
}

async fn handle_stream((mut send, mut recv): (SendStream, RecvStream)) -> Result<(), AcceptError> {
    // --- Read service name ---
    let name_len = recv.read_u8().await?;
    let mut name_buf = vec![0u8; name_len as usize];
    if let Err(e) = recv.read_exact(&mut name_buf).await {
        return Err(e!(AcceptError::User {
            source: AnyError::from_std(e)
        }));
    };

    let service = match String::from_utf8(name_buf) {
        Ok(s) => s,
        Err(e) => {
            return Err(e!(AcceptError::User {
                source: AnyError::from_std(e)
            }));
        },
    };
    let backend_addr = match service.as_str() {
        "demo3001" => "127.0.0.1:3001",
        "demo3002" => "127.0.0.1:3002",
        _ => {
            match send.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").await {
                Ok(o) => return Ok(o),
                Err(e) => {
                    return Err(e!(AcceptError::User {
                        source: AnyError::from_std(e)
                    }))
                },
            };
        },
    };

    // --- Connect to backend HTTP server ---
    let mut backend = TcpStream::connect(backend_addr).await?;

    // --- Tunnel data ---
    let mut iroh_stream = IrohStream::new(send, recv);
    match tokio::io::copy_bidirectional(&mut backend, &mut iroh_stream).await {
        Ok((client_to_backend, backend_to_client)) => {
            info!(
                "--> wrote to service {} bytes, <-- wrote back to iroh {} bytes",
                client_to_backend, backend_to_client
            );
        }
        Err(e) => {
            error!("stream error: {e:?}");
        }
    }

    Ok(())
}
