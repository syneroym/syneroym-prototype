use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Multipart, Path, State,
    },
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use wasm_service_core::*;

pub struct HttpTransport {
    runtime: Arc<WasmRuntime>,
    service_name: String,
}

impl HttpTransport {
    pub fn new(runtime: Arc<WasmRuntime>, service_name: String) -> Self {
        HttpTransport {
            runtime,
            service_name,
        }
    }

    pub async fn build_router(self) -> Router {
        let capabilities = self.runtime.discover_capabilities().await.unwrap();

        let state = Arc::new(self);
        let mut router = Router::new();

        // Register RPC methods as HTTP endpoints
        for method in capabilities.methods {
            let route = Self::method_to_http_route(&method.name);

            if method.request_streaming || method.response_streaming {
                router = router.route(&route, post(Self::handle_streaming_method));
            } else {
                router = router.route(&route, post(Self::handle_rpc_method));
            }
        }

        // Register stream types as WebSocket endpoints
        for stream_config in capabilities.streams {
            let ws_route = format!("/ws/{}", stream_config.stream_type);
            router = router.route(&ws_route, get(Self::handle_websocket));
        }

        // Fallback for unknown routes
        router = router.fallback(Self::handle_not_found);

        router.with_state(state)
    }

    fn method_to_http_route(method: &str) -> String {
        format!("/api/{}", method.replace('.', "/"))
    }

    // Simple RPC handler (no streaming)
    async fn handle_rpc_method(
        State(transport): State<Arc<HttpTransport>>,
        uri: Uri,
        _method: Method,
        headers: HeaderMap,
        body: Bytes,
    ) -> impl IntoResponse {
        let method_name = uri.path().trim_start_matches("/api/").replace('/', ".");

        let request = Self::http_to_canonical(
            method_name,
            headers,
            body.to_vec(),
            None,
            &transport.service_name,
        );

        match transport.runtime.handle_request(request).await {
            Ok(response) => Self::canonical_to_http(response),
            Err(e) => {
                tracing::error!("Runtime error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Runtime error").into_response()
            },
        }
    }

    // Streaming RPC handler (file uploads, etc.)
    async fn handle_streaming_method(
        State(transport): State<Arc<HttpTransport>>,
        uri: Uri,
        headers: HeaderMap,
        mut multipart: Multipart,
    ) -> impl IntoResponse {
        let method_name = uri.path().trim_start_matches("/api/").replace('/', ".");

        // Handle multipart upload
        if let Ok(Some(field)) = multipart.next_field().await {
            let filename = field.file_name().map(|s| s.to_string());

            let bytes = match field.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Upload error: {}", e),
                    )
                        .into_response()
                },
            };

            // Create input stream from buffered bytes
            let stream_reader = std::io::Cursor::new(bytes);

            let stream_id = transport
                .runtime
                .capabilities()
                .stream_manager
                .register_input_stream(
                    stream_reader,
                    StreamInfo {
                        id: String::new(),
                        content_type: None,
                        content_length: None,
                        filename,
                    },
                );

            let request = Self::http_to_canonical(
                method_name,
                headers,
                vec![],
                Some(stream_id),
                &transport.service_name,
            );

            match transport.runtime.handle_request(request).await {
                Ok(response) => Self::canonical_to_http(response),
                Err(e) => {
                    tracing::error!("Runtime error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Runtime error").into_response()
                },
            }
        } else {
            (StatusCode::BAD_REQUEST, "No file provided").into_response()
        }
    }

    // WebSocket handler
    async fn handle_websocket(
        ws: WebSocketUpgrade,
        Path(stream_type): Path<String>,
        State(transport): State<Arc<HttpTransport>>,
    ) -> impl IntoResponse {
        ws.on_upgrade(move |socket| {
            Self::handle_websocket_connection(socket, stream_type, transport)
        })
    }

    async fn handle_websocket_connection(
        socket: WebSocket,
        stream_type: String,
        transport: Arc<HttpTransport>,
    ) {
        let (mut ws_sender, mut ws_receiver) = socket.split();
        let stream_id = uuid::Uuid::new_v4().to_string();

        let (wasm_to_ws_tx, mut wasm_to_ws_rx) = mpsc::channel::<Vec<u8>>(100);

        // Register stream
        transport
            .runtime
            .capabilities()
            .message_streams
            .register_stream(stream_id.clone(), stream_type.clone(), wasm_to_ws_tx);

        loop {
            tokio::select! {
                // Messages from WASM to client
                Some(payload) = wasm_to_ws_rx.recv() => {
                    let text = String::from_utf8_lossy(&payload).to_string();
                    if ws_sender.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }

                // Messages from client to WASM
                Some(msg) = ws_receiver.next() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            let stream_ctx = StreamContext {
                                stream_id: stream_id.clone(),
                                stream_type: stream_type.clone(),
                                metadata: vec![],
                            };

                            if let Err(e) = transport
                                .runtime
                                .handle_stream_message(stream_ctx, text.into_bytes())
                                .await
                            {
                                tracing::error!("Stream message error: {}", e);
                            }
                        }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
            }
        }

        transport
            .runtime
            .capabilities()
            .message_streams
            .unregister_stream(&stream_type, &stream_id);
    }

    async fn handle_not_found() -> impl IntoResponse {
        (StatusCode::NOT_FOUND, "Not found")
    }

    // Convert HTTP to canonical request
    fn http_to_canonical(
        method: String,
        headers: HeaderMap,
        body: Vec<u8>,
        input_stream: Option<String>,
        service_name: &str,
    ) -> CanonicalRequest {
        let metadata: Vec<(String, String)> = headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        CanonicalRequest {
            method: method.clone(),
            payload: if body.is_empty() { None } else { Some(body) },
            input_stream,
            metadata,
            context: RequestContext {
                request_id: uuid::Uuid::new_v4().to_string(),
                service_name: service_name.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                transport_info: Some(TransportInfo {
                    protocol: "http".to_string(),
                    endpoint: method,
                }),
            },
        }
    }

    // Convert canonical to HTTP response
    fn canonical_to_http(response: CanonicalResponse) -> Response {
        let status = Self::code_to_http_status(response.code);

        if let Some(_stream_id) = response.output_stream {
            // Streaming response
            Self::stream_response(status, response.metadata)
        } else {
            // Regular response
            let mut res = Response::new(Body::from(response.payload.unwrap_or_default()));
            *res.status_mut() = status;

            for (key, value) in response.metadata {
                if let (Ok(k), Ok(v)) = (
                    axum::http::HeaderName::from_bytes(key.as_bytes()),
                    axum::http::HeaderValue::from_str(&value),
                ) {
                    res.headers_mut().insert(k, v);
                }
            }

            res
        }
    }

    fn stream_response(status: StatusCode, metadata: Vec<(String, String)>) -> Response {
        // This is a placeholder - in full implementation, we'd stream from the output stream
        let mut res = Response::new(Body::from("Streaming not fully implemented"));
        *res.status_mut() = status;

        for (key, value) in metadata {
            if let (Ok(k), Ok(v)) = (
                axum::http::HeaderName::from_bytes(key.as_bytes()),
                axum::http::HeaderValue::from_str(&value),
            ) {
                res.headers_mut().insert(k, v);
            }
        }

        res
    }

    fn code_to_http_status(code: u32) -> StatusCode {
        match code {
            0 => StatusCode::OK,
            1 => StatusCode::BAD_REQUEST,
            2 => StatusCode::NOT_FOUND,
            3 => StatusCode::INTERNAL_SERVER_ERROR,
            4 => StatusCode::UNAUTHORIZED,
            5 => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
