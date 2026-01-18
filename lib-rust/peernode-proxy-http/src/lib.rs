use axum::{
    body::Body,
    extract::{State, WebSocketUpgrade},
    http::Uri,
    response::{IntoResponse, Response},
    Router,
};
use futures::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use iroh::endpoint::{RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr};
use protocol_base::SYNEROYM_ALPN;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;
use tracing::{error, info};

type NodeId = EndpointAddr;

struct AppState {
    iroh: Endpoint,
    target: NodeId,
}

pub async fn start(port: u16, target: NodeId) -> anyhow::Result<()> {
    info!(
        "Starting PeerNode HTTP Proxy on port {}, target: {:?}",
        port, target
    );

    // Bind a new local iroh endpoint for the proxy client
    let endpoint = Endpoint::bind().await?;

    let state = Arc::new(AppState {
        iroh: endpoint,
        target,
    });

    let app = Router::new().fallback(common_handler).with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;

    info!("PeerNode HTTP Proxy listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

fn extract_service_and_path(path: &str) -> Option<(String, String)> {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        return None;
    }
    match path.split_once('/') {
        Some((service, rest)) => Some((service.to_string(), format!("/{}", rest))),
        None => Some((path.to_string(), "/".to_string())),
    }
}

fn extract_service_info_or_error(path: &str) -> Result<(String, String), Response> {
    extract_service_and_path(path).ok_or_else(|| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            "Missing service name in path",
        )
            .into_response()
    })
}

async fn common_handler(
    State(state): State<Arc<AppState>>,
    ws: Option<WebSocketUpgrade>,
    req: axum::extract::Request,
) -> Response {
    if let Some(ws) = ws {
        ws_handler(ws, State(state), req.uri().clone()).await
    } else {
        proxy_handler(State(state), req).await
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    uri: Uri,
) -> Response {
    let path = uri.path();
    let (service_name, remaining_path) = match extract_service_info_or_error(path) {
        Ok(res) => res,
        Err(e) => return e,
    };

    let full_path = if let Some(query) = uri.query() {
        format!("{}?{}", remaining_path, query)
    } else {
        remaining_path
    };

    ws.on_upgrade(move |socket| handle_ws(socket, state, service_name, full_path))
}

async fn connect_and_handshake(
    state: &AppState,
    service_name: &str,
) -> anyhow::Result<(SendStream, RecvStream)> {
    let connection = state
        .iroh
        .connect(state.target.clone(), SYNEROYM_ALPN)
        .await?;
    let (mut send, recv) = connection.open_bi().await?;

    let name = service_name.as_bytes();
    send.write_u8(name.len() as u8).await?;
    send.write_all(name).await?;

    Ok((send, recv))
}

async fn handle_ws(
    socket: axum::extract::ws::WebSocket,
    state: Arc<AppState>,
    service_name: String,
    path: String,
) {
    // Connecting to Iroh and sending Service Name
    let (mut iroh_sender, iroh_recv) = match connect_and_handshake(&state, &service_name).await {
        Ok(streams) => streams,
        Err(e) => {
            error!("Failed to connect and handshake with iroh target: {}", e);
            return;
        },
    };

    // Send Path/Handshake to backend (simulated)
    // This allows the backend to see the request path and protocol
    let handshake = format!(
        "GET {} HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        path
    );
    if let Err(e) = iroh_sender.write_all(handshake.as_bytes()).await {
        error!("Failed to write handshake: {}", e);
        return;
    }

    info!(
        "WS connection opened for service: {}, path: {}",
        service_name, path
    );

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Downstream: Iroh -> WS
    // Read bytes from Iroh and send as Binary frames to WS
    let downstream = async {
        let mut reader = ReaderStream::new(iroh_recv);
        while let Some(chunk) = reader.next().await {
            match chunk {
                Ok(bytes) => {
                    if let Err(e) = ws_sender
                        .send(axum::extract::ws::Message::Binary(bytes.into()))
                        .await
                    {
                        error!("Failed to send to WS client: {}", e);
                        break;
                    }
                },
                Err(e) => {
                    error!("Error reading from Iroh: {}", e);
                    break;
                },
            }
        }
    };

    // Upstream: WS -> Iroh
    // Read messages from WS and write payload to Iroh
    let upstream = async {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(msg) => {
                    // Extract payload bytes (Text or Binary)
                    let data = msg.into_data();
                    if !data.is_empty() {
                        if let Err(e) = iroh_sender.write_all(&data).await {
                            error!("Failed to write to Iroh: {}", e);
                            break;
                        }
                    }
                },
                Err(e) => {
                    error!("WS client error: {}", e);
                    break;
                },
            }
        }
    };

    tokio::select! {
        _ = downstream => {},
        _ = upstream => {},
    }
}

async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> Response {
    let uri = req.uri().clone();
    let path = uri.path();

    let (service_name, remaining_path) = match extract_service_info_or_error(path) {
        Ok(res) => res,
        Err(e) => return e,
    };

    // 1 & 2. Connect to Iroh Target and Send Service Name
    let (mut send, recv) = match connect_and_handshake(&state, &service_name).await {
        Ok(streams) => streams,
        Err(e) => {
            return (
                axum::http::StatusCode::BAD_GATEWAY,
                format!("Connect/Handshake error: {}", e),
            )
                .into_response()
        },
    };

    // 3. Serialize HTTP Request to Iroh Stream
    let method = req.method().as_str();
    // Use the remaining path for the forwarded request
    let path_to_forward = if let Some(query) = uri.query() {
        format!("{}?{}", remaining_path, query)
    } else {
        remaining_path
    };

    let version = match req.version() {
        axum::http::Version::HTTP_09 => "HTTP/0.9",
        axum::http::Version::HTTP_10 => "HTTP/1.0",
        axum::http::Version::HTTP_11 => "HTTP/1.1",
        axum::http::Version::HTTP_2 => "HTTP/2.0",
        axum::http::Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/1.1",
    };

    let request_line = format!("{} {} {}\r\n", method, path_to_forward, version);
    if let Err(_) = send.write_all(request_line.as_bytes()).await {
        return axum::http::StatusCode::BAD_GATEWAY.into_response();
    }

    for (name, value) in req.headers() {
        if let Ok(v) = value.to_str() {
            let header_line = format!("{}: {}\r\n", name, v);
            if let Err(_) = send.write_all(header_line.as_bytes()).await {
                return axum::http::StatusCode::BAD_GATEWAY.into_response();
            }
        }
    }
    if let Err(_) = send.write_all(b"\r\n").await {
        return axum::http::StatusCode::BAD_GATEWAY.into_response();
    }

    // 4. Stream Body
    let mut body = req.into_body();
    while let Some(chunk) = body.frame().await {
        match chunk {
            Ok(frame) => {
                if let Ok(data) = frame.into_data() {
                    if let Err(_) = send.write_all(&data).await {
                        return axum::http::StatusCode::BAD_GATEWAY.into_response();
                    }
                }
            },
            Err(_) => return axum::http::StatusCode::BAD_GATEWAY.into_response(),
        }
    }

    // 5. Read Response from Iroh Stream and pipe back to Axum Response
    parse_iroh_response(recv).await
}

async fn parse_iroh_response(recv: RecvStream) -> Response {
    let mut reader = BufReader::new(recv);
    let mut line = String::new();

    // Parse Status Line
    if reader.read_line(&mut line).await.is_err() {
        return axum::http::StatusCode::BAD_GATEWAY.into_response();
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return axum::http::StatusCode::BAD_GATEWAY.into_response();
    }

    let status_code = parts[1].parse::<u16>().unwrap_or(502);
    let status = axum::http::StatusCode::from_u16(status_code)
        .unwrap_or(axum::http::StatusCode::BAD_GATEWAY);

    let mut builder = Response::builder().status(status);

    // Parse Headers
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                if line == "\r\n" || line == "\n" {
                    break;
                }

                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();
                    builder = builder.header(key, value);
                }
            }
            Err(_) => return axum::http::StatusCode::BAD_GATEWAY.into_response(),
        }
    }

    let stream = tokio_util::io::ReaderStream::new(reader);
    let body = Body::from_stream(stream);

    builder
        .body(body)
        .unwrap_or_else(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
