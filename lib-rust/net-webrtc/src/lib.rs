use anyhow::Result;
use common::config::Config;
use protocol_base::ProtocolHandler;
use std::sync::Arc;
use tracing::{debug, info};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;

pub async fn init(config: &Config, _handlers: Vec<Arc<dyn ProtocolHandler>>) -> Result<()> {
    if let Some(_webrtc_config) = &config.comm_webrtc {
        info!("Initializing WebRTC communication...");

        let mut m = MediaEngine::default();
        m.register_default_codecs()?;

        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)?;

        let _api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .build();

        let _config = RTCConfiguration {
            ice_servers: vec![webrtc::ice_transport::ice_server::RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };

        // TODO: Connect to Signaling Server using webrtc_config.signaling_server_url
        // For now, this is a placeholder to show where the logic would sit.
        // We would receive an Offer, create a PeerConnection, SetRemoteDescription, CreateAnswer, etc.

        // Example of what we would do on a new PeerConnection:
        // pc.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
        //     let handlers = handlers.clone();
        //     Box::pin(async move {
        //         handle_data_channel(d, handlers).await;
        //     })
        // }));

        info!("WebRTC stack initialized (signaling pending).");
    }
    Ok(())
}

async fn handle_data_channel(d: Arc<RTCDataChannel>, _handlers: Vec<Arc<dyn ProtocolHandler>>) {
    let d_label = d.label().to_owned();
    let d_id = d.id();
    info!("New DataChannel {} {}", d_label, d_id);

    // Clone d for the closure
    let d2 = d.clone();

    // Register on_message handler
    d.on_message(Box::new(move |msg: DataChannelMessage| {
        let d = d2.clone();
        let d_label = d.label().to_owned();
        Box::pin(async move {
            let data = msg.data;
            info!("Received {} bytes on DataChannel '{}'", data.len(), d_label);

            // Simplified handling:
            // In a full implementation, we would treat this as a stream or packet source
            // and forward to the service.
            // Since we can't easily get a AsyncRead/AsyncWrite wrapper (PollDataChannel) 
            // without correct imports, we demonstrate the logic here:

            let service_name_len = if !data.is_empty() { data[0] as usize } else { 0 };
            if data.len() > service_name_len + 1 {
                 let service_name = String::from_utf8_lossy(&data[1..1+service_name_len]);
                 debug!("Potential service request for: {}", service_name);
                 
                 // If we could forward:
                 // let backend_addr = match service_name.as_ref() { "demo3001" => "127.0.0.1:3001", ... }
                 // let mut stream = TcpStream::connect(backend_addr).await...
                 // stream.write_all(&data[1+service_name_len..]).await...
            }
        })
    }));
}
