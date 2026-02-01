use anyhow::{Result, anyhow};
use askama::Template;
use common::iroh_utils::IrohStream;
use common::protocol_utils::{
    extract_host_from_http, extract_service_from_host, extract_sni, is_tls_client_hello,
};
use iroh::{Endpoint, EndpointAddr};
use protocol_base::SYNEROYM_ALPN;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{self, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

#[derive(Clone)]
struct AppState {
    iroh: Endpoint,
    target: EndpointAddr,
    signaling_server_url: String,
}

#[derive(Template)]
#[template(path = "peer-proxy.html")]
struct PeerProxyTemplate<'a> {
    signaling_server_url: &'a str,
    target_peer_id: &'a str,
    http_version: &'a str,
}

#[derive(Template)]
#[template(path = "sw.js", escape = "none")]
struct SwTemplate;

pub async fn start(
    port: u16,
    target: EndpointAddr,
    signaling_server_url: String,
    iroh_relay_url: Option<String>,
) -> Result<()> {
    info!(
        "Starting LocalNode Web Gateway on port {}, target: {:?}",
        port, target
    );

    let endpoint = common::iroh_utils::bind_endpoint(iroh_relay_url).await?;

    let state = Arc::new(AppState {
        iroh: endpoint,
        target,
        signaling_server_url,
    });

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    info!("LocalNode Web Gateway listening on {}", addr);

    loop {
        let (client, addr) = listener.accept().await?;
        debug!("New connection from: {}", addr);
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(client, state).await {
                debug!("Connection error: {}", e);
            }
        });
    }
}

async fn handle_connection(mut client: TcpStream, state: Arc<AppState>) -> Result<()> {
    let mut peek_buf = vec![0u8; 4096];
    let n = client.peek(&mut peek_buf).await?;
    if n == 0 {
        return Ok(());
    }

    // 1. Try TLS
    if is_tls_client_hello(&peek_buf[..n]) {
        debug!("Detected TLS connection");
        let hostname = extract_sni(&peek_buf[..n])?;
        return tunnel_to_iroh(client, &hostname, state).await;
    }

    // 2. Try HTTP
    let http_info = parse_http_peek(&peek_buf[..n]);

    if let Ok((_method, path, host, has_loop_header, is_websocket)) = http_info {
        // debug!("Detected HTTP: {} {} (Host: {}, WS: {})", _method, path, host, is_websocket);

        if has_loop_header {
            let resp = "HTTP/1.1 502 Bad Gateway\r\nX-Peer-Proxy-Error: Loop Detected\r\nContent-Length: 13\r\n\r\nLoop Detected";
            client.write_all(resp.as_bytes()).await?;
            return Ok(());
        }

        if is_websocket {
            // Tunnel WebSockets
            debug!("Tunneling WebSocket request for host: {}", host);
            return tunnel_to_iroh(client, &host, state).await;
        }

        if path == "/__syneroym/sw.js" {
            // Serve Service Worker
            return serve_sw(client).await;
        }

        // For all other requests (Navigation or otherwise), serve the index shell
        // This allows the Service Worker to take over via the shell.
        return serve_index(client, &host, state).await;
    }

    // 3. Fallback: just try to extract host (maybe it was partial HTTP or something)
    // or fail.
    match extract_host_from_http(&peek_buf[..n]) {
        Ok(host) => {
            debug!("Fallback: Extracted host {}, tunneling", host);
            tunnel_to_iroh(client, &host, state).await
        }
        Err(_) => {
            // Could not identify protocol or host
            Err(anyhow!(
                "Could not identify protocol or host from peeked data"
            ))
        }
    }
}

async fn serve_index(mut client: TcpStream, host: &str, state: Arc<AppState>) -> Result<()> {
    let peer_id = extract_peer_id_from_host(host);
    let template = PeerProxyTemplate {
        signaling_server_url: &state.signaling_server_url,
        target_peer_id: &peer_id,
        http_version: "HTTP/1.1",
    };

    match template.render() {
        Ok(content) => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                content.len(),
                content
            );
            client.write_all(response.as_bytes()).await?;
        }
        Err(e) => {
            error!("Template render error: {}", e);
            let resp = "HTTP/1.1 500 Internal Server Error\r\n\r\n";
            client.write_all(resp.as_bytes()).await?;
        }
    }
    Ok(())
}

async fn serve_sw(mut client: TcpStream) -> Result<()> {
    let template = SwTemplate;
    match template.render() {
        Ok(content) => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/javascript\r\nService-Worker-Allowed: /\r\nContent-Length: {}\r\n\r\n{}",
                content.len(),
                content
            );
            client.write_all(response.as_bytes()).await?;
        }
        Err(e) => {
            error!("Template render error: {}", e);
            let resp = "HTTP/1.1 500 Internal Server Error\r\n\r\n";
            client.write_all(resp.as_bytes()).await?;
        }
    }
    Ok(())
}

async fn tunnel_to_iroh(mut client: TcpStream, hostname: &str, state: Arc<AppState>) -> Result<()> {
    let svc_name = extract_service_from_host(hostname)?;
    debug!("Tunneling to service: {}", svc_name);

    // Connect to Iroh
    let connection = state
        .iroh
        .connect(state.target.clone(), SYNEROYM_ALPN)
        .await?;
    let (send, recv) = connection.open_bi().await?;

    // Handshake
    let svc_raw = svc_name.as_bytes();
    let mut iroh_stream = IrohStream::new(send, recv);
    iroh_stream.write_u8(svc_raw.len() as u8).await?;
    iroh_stream.write_all(svc_raw).await?;

    // Proxy
    let (c2s, s2c) = io::copy_bidirectional(&mut client, &mut iroh_stream).await?;
    debug!(
        "Tunnel finished: client->server={}, server->client={}",
        c2s, s2c
    );
    Ok(())
}

// Helpers

fn parse_http_peek(buf: &[u8]) -> Result<(String, String, String, bool, bool)> {
    let text = String::from_utf8_lossy(buf);
    let mut lines = text.lines();

    // Request line
    let req_line = lines.next().ok_or_else(|| anyhow!("Empty request"))?;
    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(anyhow!("Invalid request line"));
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    let mut host = String::new();
    let mut has_loop = false;
    let mut is_websocket = false;

    for line in lines {
        if line.len() > 5 && line[..5].eq_ignore_ascii_case("host:") {
            let h = line[5..].trim();
            host = h.split(':').next().unwrap_or(h).to_string();
        } else if line.len() > 12 && line[..12].eq_ignore_ascii_case("x-peer-proxy") {
            has_loop = true;
        } else if line.len() > 8 && line[..8].eq_ignore_ascii_case("upgrade:") {
            let val = line[8..].trim();
            if val.eq_ignore_ascii_case("websocket") {
                is_websocket = true;
            }
        }
        // Break on empty line? Not strictly necessary for peek parsing
    }

    if host.is_empty() {
        return Err(anyhow!("No Host header"));
    }

    Ok((method, path, host, has_loop, is_websocket))
}

fn extract_peer_id_from_host(host: &str) -> String {
    let hostname = host.split(':').next().unwrap_or(host);
    match hostname.split_once('.') {
        Some((_, rest)) => rest.to_string(),
        None => hostname.to_string(),
    }
}
