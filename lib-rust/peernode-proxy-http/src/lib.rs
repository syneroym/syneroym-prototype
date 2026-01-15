use axum::{
    body::Body,
    extract::{State, WebSocketUpgrade},
    response::{IntoResponse, Response},
    routing::any,
    Router,
    http::Uri,
};
use http_body_util::BodyExt;
use iroh::{Endpoint, PublicKey};
use iroh::endpoint::{Connection, RecvStream, SendStream};
use protocol_base::SYNEROYM_ALPN;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tracing::{error, info};

type NodeId = PublicKey;

struct AppState {
    iroh: Endpoint,
    target: NodeId,
}

pub async fn start(port: u16, target: NodeId) -> anyhow::Result<()> {
    info!("Starting PeerNode HTTP Proxy on port {}, target: {}", port, target);
    
    // Bind a new local iroh endpoint for the proxy client
    let endpoint = Endpoint::bind().await?;
    
    let state = Arc::new(AppState {
        iroh: endpoint,
        target,
    });

    let app = Router::new()
        .route("/ws", any(ws_handler))
        .fallback(proxy_handler)
        .with_state(state);

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

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    uri: Uri,
) -> Response {
    let path = uri.path();
    let service_name = match extract_service_and_path(path) {
        Some((s, _)) => s, // For WS, we just need the service name to connect.
                           // The path might be relevant if the backend needs it,
                           // but for now we just connect to the service.
                           // If we need to send the sub-path to the backend,
                           // we would need to modify the protocol to send path info.
                           // The current requirement is "send service name".
                           // Assuming the backend handles the stream from there.
                           // Wait, for HTTP proxy we send the request line.
                           // For WS, we just send service name and then tunnel?
                           // The prompt says "Need to send over this service_name and length."
                           // It doesn't explicitly say we need to send the path for WS.
                           // But usually WS connection URL matters.
                           // However, `handle_ws` implementation below sends service name then pumps bytes.
                           // It doesn't send a synthetic HTTP request like `proxy_handler`.
                           // So I'll just extract the service name.
        None => return (axum::http::StatusCode::BAD_REQUEST, "Missing service name in path").into_response(),
    };

    ws.on_upgrade(move |socket| handle_ws(socket, state, service_name))
}

async fn handle_ws(mut socket: axum::extract::ws::WebSocket, state: Arc<AppState>, service_name: String) {
    // Connecting to Iroh
    let connection: Connection = match state.iroh.connect(state.target, SYNEROYM_ALPN).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to connect to iroh target: {}", e);
            return;
        }
    };
    
    let (mut send, _recv): (SendStream, RecvStream) = match connection.open_bi().await {
        Ok(bi) => bi,
        Err(e) => {
            error!("Failed to open bi stream: {}", e);
            return;
        }
    };

    // Send Service Name
    let name = service_name.as_bytes();
    if let Err(e) = send.write_u8(name.len() as u8).await {
        error!("Failed to write service name len: {}", e);
        return;
    }
    if let Err(e) = send.write_all(name).await {
        error!("Failed to write service name: {}", e);
        return;
    }

    info!("WS connection opened for service: {}", service_name);
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
             info!("Received WS msg: {:?}", msg);
             // Placeholder for forwarding
        } else {
            break;
        }
    }
}

async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> Response {
    let uri = req.uri().clone();
    let path = uri.path();
    
    let (service_name, remaining_path) = match extract_service_and_path(path) {
        Some(res) => res,
        None => return (axum::http::StatusCode::BAD_REQUEST, "Missing service name in path").into_response(),
    };

    // 1. Connect to Iroh Target
    let connection: Connection = match state.iroh.connect(state.target, SYNEROYM_ALPN).await {
        Ok(c) => c,
        Err(e) => return (axum::http::StatusCode::BAD_GATEWAY, format!("Connect error: {}", e)).into_response(),
    };

    let (mut send, recv): (SendStream, RecvStream) = match connection.open_bi().await {
        Ok(bi) => bi,
        Err(e) => return (axum::http::StatusCode::BAD_GATEWAY, format!("Stream error: {}", e)).into_response(),
    };

    // 2. Send Service Name
    let name = service_name.as_bytes();
    if let Err(e) = send.write_u8(name.len() as u8).await {
         return (axum::http::StatusCode::BAD_GATEWAY, format!("Write error: {}", e)).into_response();
    }
    if let Err(e) = send.write_all(name).await {
         return (axum::http::StatusCode::BAD_GATEWAY, format!("Write error: {}", e)).into_response();
    }

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
    let stream = tokio_util::io::ReaderStream::new(recv);
    let body = Body::from_stream(stream);

    Response::new(body)
}
