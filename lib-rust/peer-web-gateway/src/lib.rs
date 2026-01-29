use askama::Template;
use axum::{
    Router,
    extract::{
        Host, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use common::stream::IrohStream;
use futures::{SinkExt, StreamExt};
use iroh::{Endpoint, EndpointAddr};
use protocol_base::SYNEROYM_ALPN;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, error, info};

type NodeId = EndpointAddr;

#[derive(Clone)]
struct AppState {
    iroh: Endpoint,
    target: NodeId,
    signaling_server_url: String,
}

pub async fn start(port: u16, target: NodeId, signaling_server_url: String) -> anyhow::Result<()> {
    info!(
        "Starting LocalNode Web Gateway on port {}, target: {:?}",
        port, target
    );

    let endpoint = Endpoint::bind().await?;

    let state = AppState {
        iroh: endpoint,
        target,
        signaling_server_url,
    };

    let app = Router::new()
        .route("/sw.js", get(sw_handler))
        .fallback(index_handler)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("LocalNode Web Gateway listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
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

async fn index_handler(
    State(state): State<AppState>,
    ws: Option<WebSocketUpgrade>,
    Host(host): Host,
    headers: HeaderMap,
) -> Response {
    debug!("Received index request for host: {}", host);

    // Loop Protection
    if headers.contains_key("X-Peer-Proxy") {
        return (
            StatusCode::BAD_GATEWAY,
            "Error: Request reached gateway server. Service Worker failed to intercept.",
        )
            .into_response();
    }

    if let Some(ws) = ws {
        debug!("Upgrading to WebSocket for host: {}", host);
        return ws.on_upgrade(move |socket| handle_socket(socket, state, host));
    }

    let peer_id = extract_peer_id_from_host(&host);

    let template = PeerProxyTemplate {
        signaling_server_url: &state.signaling_server_url,
        target_peer_id: &peer_id,
        http_version: "HTTP/1.1",
    };

    match template.render() {
        Ok(content) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(content.into())
            .unwrap(),
        Err(e) => {
            tracing::error!("Index template rendering failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}

async fn sw_handler(State(_state): State<AppState>) -> Response {
    debug!("Received SW request");

    let template = SwTemplate;

    match template.render() {
        Ok(content) => Response::builder()
            .header(header::CONTENT_TYPE, "application/javascript")
            .header("Service-Worker-Allowed", "/")
            .body(content.into())
            .unwrap(),
        Err(e) => {
            tracing::error!("SW template rendering failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}

async fn handle_socket(socket: WebSocket, state: AppState, host: String) {
    let svc_name = match extract_service_from_host(&host) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to extract service name: {}", e);
            return;
        }
    };

    debug!("Connecting to Iroh target for service: {}", svc_name);

    match state.iroh.connect(state.target, SYNEROYM_ALPN).await {
        Ok(connection) => {
            match connection.open_bi().await {
                Ok((send, recv)) => {
                    let svc_raw = svc_name.as_bytes();
                    let mut iroh_stream = IrohStream::new(send, recv);

                    // Handshake
                    if let Err(e) = iroh_stream.write_u8(svc_raw.len() as u8).await {
                        error!("Failed to write service len: {}", e);
                        return;
                    }
                    if let Err(e) = iroh_stream.write_all(svc_raw).await {
                        error!("Failed to write service name: {}", e);
                        return;
                    }

                    // Proxy loop
                    let (mut sender, mut receiver) = socket.split();
                    let (mut iroh_reader, mut iroh_writer) = tokio::io::split(iroh_stream);

                    let iroh_to_ws = async {
                        let mut buf = [0u8; 4096];
                        loop {
                            match iroh_reader.read(&mut buf).await {
                                Ok(0) => break, // EOF
                                Ok(n) => {
                                    if sender
                                        .send(Message::Binary(buf[..n].to_vec()))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("Error reading from Iroh: {}", e);
                                    break;
                                }
                            }
                        }
                    };

                    let ws_to_iroh = async {
                        while let Some(msg) = receiver.next().await {
                            match msg {
                                Ok(Message::Binary(data)) => {
                                    if iroh_writer.write_all(&data).await.is_err() {
                                        break;
                                    }
                                }
                                Ok(Message::Text(text)) => {
                                    if iroh_writer.write_all(text.as_bytes()).await.is_err() {
                                        break;
                                    }
                                }
                                Ok(Message::Close(_)) => break,
                                _ => {} // Ping/Pong
                            }
                        }
                    };

                    tokio::select! {
                        _ = iroh_to_ws => {},
                        _ = ws_to_iroh => {},
                    }
                    debug!("Proxy connection closed for {}", svc_name);
                }
                Err(e) => error!("Failed to open bi stream: {}", e),
            }
        }
        Err(e) => error!("Failed to connect to iroh target: {}", e),
    }
}

fn extract_peer_id_from_host(host: &str) -> String {
    let hostname = host.split(':').next().unwrap_or(host);
    match hostname.split_once('.') {
        Some((_, rest)) => rest.to_string(),
        None => hostname.to_string(),
    }
}

fn extract_service_from_host(host: &str) -> anyhow::Result<String> {
    let hostname = host.split(':').next().unwrap_or(host);
    let parts: Vec<&str> = hostname.split('.').collect();
    if parts.len() > 1 {
        Ok(parts[0].to_string())
    } else {
        anyhow::bail!("service name not found in host: {}", host)
    }
}
