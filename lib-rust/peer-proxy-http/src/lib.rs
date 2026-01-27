use anyhow::{anyhow, Result};
use common::stream::IrohStream;
use iroh::{Endpoint, EndpointAddr};
use protocol_base::SYNEROYM_ALPN;
use std::net::SocketAddr;
use std::sync::Arc;

use tls_parser::{parse_tls_plaintext, TlsMessage, TlsMessageHandshake};
use tokio::io::{self, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, error, info};

type NodeId = EndpointAddr;

struct AppState {
    iroh: Endpoint,
    target: NodeId,
}

pub async fn start(port: u16, target: NodeId) -> anyhow::Result<()> {
    info!(
        "Starting LocalNode HTTP Proxy on port {}, target: {:?}",
        port, target
    );

    let endpoint = Endpoint::bind().await?;

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

fn is_tls_client_hello(buf: &[u8]) -> bool {
    // TLS record starts with:
    // - 0x16 (Handshake)
    // - 0x03 0x00 to 0x03 0x03 (SSL/TLS version)
    buf.len() >= 3 && buf[0] == 0x16 && buf[1] == 0x03
}

fn extract_sni(buf: &[u8]) -> Result<String> {
    let (_, tls_record) =
        parse_tls_plaintext(buf).map_err(|e| anyhow!("Failed to parse TLS: {:?}", e))?;

    // Look for ClientHello message
    for msg in tls_record.msg {
        if let TlsMessage::Handshake(handshake) = msg {
            if let TlsMessageHandshake::ClientHello(client_hello) = handshake {
                // Parse extensions from raw bytes
                if let Some(ext_bytes) = client_hello.ext {
                    // Use parse_tls_extensions to parse the extension bytes
                    match tls_parser::parse_tls_extensions(ext_bytes) {
                        Ok((_, extensions)) => {
                            for ext in extensions {
                                if let tls_parser::TlsExtension::SNI(sni_list) = ext {
                                    if !sni_list.is_empty() {
                                        // SNI entry is (type, hostname_bytes)
                                        let hostname = std::str::from_utf8(sni_list[0].1)
                                            .map_err(|e| anyhow!("Invalid SNI hostname: {}", e))?;
                                        return Ok(hostname.to_string());
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            error!("Failed to parse TLS extensions: {:?}", e);
                        },
                    }
                }
            }
        }
    }

    Err(anyhow!("No SNI found in TLS ClientHello"))
}
fn extract_host_from_http(buf: &[u8]) -> Result<String> {
    // Use lossy conversion to handle potential binary body data in the peek buffer
    let http_text = String::from_utf8_lossy(buf);

    // Parse HTTP headers line by line
    for line in http_text.lines() {
        if line.len() > 5 && line[..5].eq_ignore_ascii_case("host:") {
            let host = line[5..].trim();
            // Remove port if present
            let hostname = host.split(':').next().unwrap_or(host);
            return Ok(hostname.to_string());
        }
    }

    Err(anyhow!("No Host header found in HTTP request"))
}

fn extract_service_from_host(host: &str) -> Result<String> {
    let hostname = host.split(':').next().unwrap_or(host);
    let parts: Vec<&str> = hostname.split('.').collect();
    if parts.len() > 1 {
        Ok(parts[0].to_string())
    } else {
        Err(anyhow!("service name not found in host: {}", host))
    }
}
