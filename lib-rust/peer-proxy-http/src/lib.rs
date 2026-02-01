use anyhow::anyhow;
use common::iroh_utils::IrohStream;
use common::protocol_utils::{
    extract_host_from_http, extract_service_from_host, extract_sni, is_tls_client_hello,
};
use iroh::{Endpoint, EndpointAddr};
use protocol_base::SYNEROYM_ALPN;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{self, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, info};

type NodeId = EndpointAddr;

struct AppState {
    iroh: Endpoint,
    target: NodeId,
}

pub async fn start(port: u16, target: NodeId, iroh_relay_url: Option<String>) -> anyhow::Result<()> {
    info!(
        "Starting LocalNode HTTP Proxy on port {}, target: {:?}",
        port, target
    );

    let endpoint = common::iroh_utils::bind_endpoint(iroh_relay_url).await?;

    let state = Arc::new(AppState {
        iroh: endpoint,
        target,
    });

    let pxy_addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(pxy_addr).await?;

    info!("LocalNode HTTP Proxy listening on {}", pxy_addr);

    loop {
        let (client, cl_addr) = listener.accept().await?;
        debug!("New connection from: {}", cl_addr);
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = proxy_connection(client, state).await {
                debug!("connection error: {e}");
            }
        });
    }
}

async fn proxy_connection(
    mut client: tokio::net::TcpStream,
    state: Arc<AppState>,
) -> anyhow::Result<()> {
    // Peek to determine protocol and extract hostname
    let mut peek_buf = vec![0u8; 4096];
    let n = client.peek(&mut peek_buf).await?;

    if n == 0 {
        return Err(anyhow!("Client closed connection"));
    }

    // Determine if this is TLS or plain HTTP
    let hostname = if is_tls_client_hello(&peek_buf[..n]) {
        debug!("Detected TLS connection");
        extract_sni(&peek_buf[..n])?
    } else {
        debug!("Detected plain HTTP connection");
        extract_host_from_http(&peek_buf[..n])?
    };

    debug!("Extracted hostname: {}", hostname);
    let svc_name = extract_service_from_host(hostname.as_str())?;
    debug!("Extracted service name: {}", svc_name);

    // 1. Connect to Iroh
    let connection = state
        .iroh
        .connect(state.target.clone(), SYNEROYM_ALPN)
        .await?;
    let (send, recv) = connection.open_bi().await?;

    // 2. Handshake (send service name)
    let svc_raw = svc_name.as_bytes();
    let mut iroh_stream = IrohStream::new(send, recv);
    iroh_stream.write_u8(svc_raw.len() as u8).await?;
    iroh_stream.write_all(svc_raw).await?;

    // Bidirectional streaming - copies all bytes in both directions
    let (client_to_backend, backend_to_client) =
        io::copy_bidirectional(&mut client, &mut iroh_stream).await?;
    debug!(
        "proxy copied bytes {}&{}",
        client_to_backend, backend_to_client
    );

    Ok(())
}

