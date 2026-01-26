use anyhow::Result;
use common::config::Config;
use futures::{SinkExt, StreamExt};
use protocol_base::ProtocolHandler;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tracing::{debug, error, info, warn};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

// Use the external crate
use signaling_server;

pub async fn init(config: &Config, handlers: Vec<Arc<dyn ProtocolHandler>>) -> Result<()> {
    if let Some(webrtc_config) = &config.comm_webrtc {
        info!("Initializing WebRTC communication...");

        // 1. Start Signaling Server (hosted by this node for this demo)
        // Check if we should start it based on config URL or defaults.
        // For now, always start it on port 8000 if not specified, or parse the URL.
        let signaling_url = webrtc_config
            .signaling_server_url
            .clone()
            .unwrap_or_else(|| "ws://localhost:8000/ws".to_string());

        if signaling_url.contains("localhost") || signaling_url.contains("127.0.0.1") {
            // naive check: if we are pointing to localhost, assume we need to start it.
            let port = 8000; // extract from URL in real impl
            info!("Starting embedded signaling server on port {}", port);
            tokio::spawn(async move {
                signaling_server::start_server(port).await;
            });
            // Give it a moment to start
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // 2. Initialize WebRTC API
        let mut m = MediaEngine::default();
        m.register_default_codecs()?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let config = RTCConfiguration {
            ice_servers: vec![webrtc::ice_transport::ice_server::RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // 3. Connect to Signaling Server and handle incoming connections
        let api = Arc::new(api);
        let config = config.clone();
        let handlers = handlers.clone();

        tokio::spawn(async move {
            if let Err(e) = connect_signaling(signaling_url, api, config, handlers).await {
                error!("Signaling client error: {:?}", e);
            }
        });

        info!("WebRTC stack initialized.");
    }
    Ok(())
}

async fn connect_signaling(
    url: String,
    api: Arc<webrtc::api::API>,
    config: RTCConfiguration,
    handlers: Vec<Arc<dyn ProtocolHandler>>,
) -> Result<()> {
    info!("Connecting to signaling server at {}", url);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Register
    let peer_id = uuid::Uuid::new_v4().to_string();
    let register_msg = serde_json::json!({
        "type": "register",
        "id": peer_id
    });
    write
        .send(tokio_tungstenite::tungstenite::Message::Text(
            register_msg.to_string().into(),
        ))
        .await?;
    info!("Registered with signaling server as {}", peer_id);

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                error!("WebSocket error: {:?}", e);
                break;
            },
        };

        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
            let v: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let type_str = match v["type"].as_str() {
                Some(s) => s,
                None => continue,
            };

            match type_str {
                "offer" => {
                    info!("Received Offer from {:?}", v["sender"]);
                    let sdp = match v["sdp"].as_str() {
                        Some(s) => s,
                        None => continue,
                    };

                    let sender_id = v["sender"].as_str().unwrap_or("unknown");

                    // Create new PeerConnection
                    let pc = api.new_peer_connection(config.clone()).await?;

                    // TODO: Handle ICE candidates
                    // In a final app, we would send candidates back to sender_id.
                    // For now, we skip trickling or assume complete SDP if possible,
                    // or implement on_ice_candidate -> send via WS.

                    // Set Data Channel handler
                    let handlers_clone = handlers.clone();
                    pc.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
                        let handlers = handlers_clone.clone();
                        Box::pin(async move {
                            handle_data_channel(d, handlers).await;
                        })
                    }));

                    pc.on_peer_connection_state_change(Box::new(
                        move |s: RTCPeerConnectionState| {
                            info!("Peer Connection State has changed: {}", s);
                            if s == RTCPeerConnectionState::Failed {
                                // TODO cleanup
                            }
                            Box::pin(async {})
                        },
                    ));

                    // Set Remote Description
                    let desc = RTCSessionDescription::offer(sdp.to_string())?;
                    pc.set_remote_description(desc).await?;

                    // Create Answer
                    let answer = pc.create_answer(None).await?;
                    pc.set_local_description(answer.clone()).await?;

                    // Send Answer back
                    let answer_msg = serde_json::json!({
                        "type": "answer",
                        "target": sender_id,
                        "sender": peer_id,
                        "sdp": answer.sdp
                    });
                    write
                        .send(tokio_tungstenite::tungstenite::Message::Text(
                            answer_msg.to_string().into(),
                        ))
                        .await?;
                    info!("Sent Answer to {}", sender_id);
                },
                _ => {
                    debug!("Unhandled signaling message: {}", type_str);
                },
            }
        }
    }

    Ok(())
}

async fn handle_data_channel(d: Arc<RTCDataChannel>, _handlers: Vec<Arc<dyn ProtocolHandler>>) {
    let d_label = d.label().to_owned();
    let d_id = d.id();
    info!("New DataChannel {} {}", d_label, d_id);

    let d2 = d.clone();
    d.on_message(Box::new(move |msg: DataChannelMessage| {
        let d = d2.clone();
        let d_label = d.label().to_owned();
        Box::pin(async move {
            let data = msg.data;
            info!("Received {} bytes on DataChannel '{}'", data.len(), d_label);

            let service_name_len = if !data.is_empty() {
                data[0] as usize
            } else {
                0
            };
            if data.len() > service_name_len + 1 {
                let service_name = String::from_utf8_lossy(&data[1..1 + service_name_len]);
                debug!("Service request for: {}", service_name);

                let backend_addr = match service_name.as_ref() {
                    "demo3001" => "127.0.0.1:3001",
                    "demo3002" => "127.0.0.1:3002",
                    _ => {
                        warn!("Unknown service: {}", service_name);
                        return;
                    },
                };

                let payload = &data[1 + service_name_len..];

                match TcpStream::connect(backend_addr).await {
                    Ok(mut stream) => {
                        // Write payload to backend
                        if let Err(e) = stream.write_all(payload).await {
                            error!("Failed to write to backend: {}", e);
                            return;
                        }

                        // Read response from backend and send back to DataChannel
                        let mut buffer = vec![0; 4096];
                        loop {
                            match stream.read(&mut buffer).await {
                                Ok(0) => break, // EOF
                                Ok(n) => {
                                    let chunk = bytes::Bytes::copy_from_slice(&buffer[..n]);
                                    if let Err(e) = d.send(&chunk).await {
                                        error!("Failed to send back to DataChannel: {}", e);
                                        break;
                                    }
                                },
                                Err(e) => {
                                    error!("Failed to read from backend: {}", e);
                                    break;
                                },
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to connect to backend {}: {}", backend_addr, e);
                    },
                }
            }
        })
    }));
}
