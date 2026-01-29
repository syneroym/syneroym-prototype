use anyhow::Result;
use common::config::Config;
use futures::{SinkExt, StreamExt};
use protocol_base::ProtocolHandler;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tracing::{debug, error, info, warn};
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::data_channel::RTCDataChannel;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

// Use the external crate

mod stream;
use stream::WebRTCStream;

pub async fn init(config: &Config, handlers: Vec<Arc<dyn ProtocolHandler>>) -> Result<()> {
    if let Some(webrtc_config) = &config.comm_webrtc {
        info!("Initializing WebRTC communication...");

        let signaling_url = webrtc_config
            .signaling_server_url
            .clone()
            .unwrap_or_else(|| "ws://localhost:8000/ws".to_string());

        // 2. Initialize WebRTC API
        let mut m = MediaEngine::default();
        m.register_default_codecs()?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let rtc_config = RTCConfiguration {
            ice_servers: vec![webrtc::ice_transport::ice_server::RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // 3. Connect to Signaling Server and handle incoming connections
        let api = Arc::new(api);
        let rtc_config = rtc_config.clone();
        let handlers = handlers.clone();

        let peer_id = "localhost".to_string(); // TODO use the node ID hash after standardizing that. 

        tokio::spawn(async move {
            if let Err(e) =
                connect_signaling(peer_id, signaling_url, api, rtc_config, handlers).await
            {
                error!("Signaling client error: {:?}", e);
            }
        });

        info!("WebRTC stack initialized.");
    }
    Ok(())
}

async fn connect_signaling(
    peer_id: String,
    url: String,
    api: Arc<webrtc::api::API>,
    config: RTCConfiguration,
    handlers: Vec<Arc<dyn ProtocolHandler>>,
) -> Result<()> {
    info!("Connecting to signaling server at {}", url);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Register
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
            }
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
                }
                _ => {
                    debug!("Unhandled signaling message: {}", type_str);
                }
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
    d.on_open(Box::new(move || {
        let d = d2.clone();
        let d_label = d_label.clone();
        Box::pin(async move {
            info!("DataChannel '{}' open", d_label);

            match d.detach().await {
                Ok(rtc_detached) => {
                    info!("DataChannel '{}' detached successfully", d_label);

                    let mut rtc_stream = WebRTCStream::new(rtc_detached);

                    // 1. Read Preamble: [1 byte len]
                    let service_name_len = match rtc_stream.read_u8().await {
                        Ok(n) => n as usize,
                        Err(e) => {
                            error!("Failed to read length: {}", e);
                            return;
                        }
                    };

                    // 2. Read Service Name
                    let mut name_buf = vec![0u8; service_name_len];
                    if let Err(e) = rtc_stream.read_exact(&mut name_buf).await {
                        error!("Failed to read service name: {}", e);
                        return;
                    }

                    let service_name = String::from_utf8_lossy(&name_buf);
                    debug!("Service request for: {}", service_name);

                    let backend_addr = match service_name.as_ref() {
                        "demo3001" => "127.0.0.1:3001",
                        "demo3002" => "127.0.0.1:3002",
                        _ => {
                            warn!("Unknown service: {}", service_name);
                            return;
                        }
                    };

                    match TcpStream::connect(backend_addr).await {
                        Ok(mut backend_stream) => {
                            info!("Connected to backend {}, streaming data...", backend_addr);

                            match tokio::io::copy_bidirectional(
                                &mut rtc_stream,
                                &mut backend_stream,
                            )
                            .await
                            {
                                Ok((client_to_backend, backend_to_client)) => {
                                    debug!(
                                        "Streaming finished for {}: sent {}, received {}",
                                        d_label, client_to_backend, backend_to_client
                                    );
                                }
                                Err(e) => {
                                    debug!("Streaming error/end for {}: {}", d_label, e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to connect to backend {}: {}", backend_addr, e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to detach DataChannel '{}': {}", d_label, e);
                }
            }
        })
    }));
}
