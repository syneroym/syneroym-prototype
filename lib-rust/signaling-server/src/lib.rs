use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tracing::{info, warn};

// A simple signaling server state
struct AppState {
    // Map of connected peers: PeerID -> Tx channel
    peers: Mutex<HashMap<String, broadcast::Sender<String>>>,
}

pub async fn start_server(port: u16) {
    let state = Arc::new(AppState {
        peers: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Signaling server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // 1. Wait for "register" message or just generate an ID?
    // For simplicity, let's assume the first message contains the Peer ID.
    // Or we just broadcast everything to everyone (simple signaling for 2 peers).
    // Let's go with a simple "broadcast to all other peers" model for this demo,
    // or a simple ID-based routing if the message has a "target" field.

    // We'll use a broadcast channel for this connection so we can subscribe to messages from others.
    let (tx, mut rx) = broadcast::channel(100);

    // Perform a simple handshake: wait for {"type": "register", "id": "my-id"}
    let peer_id = if let Some(Ok(Message::Text(text))) = receiver.next().await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if v["type"] == "register" {
                v["id"].as_str().map(|s| s.to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let peer_id = match peer_id {
        Some(id) => id,
        None => {
            warn!("Client did not register correctly. Closing.");
            return;
        },
    };

    info!("Peer registered: {}", peer_id);

    {
        let mut peers = state.peers.lock().unwrap();
        peers.insert(peer_id.clone(), tx.clone());
    }

    // Spawn a task to forward messages from the broadcast channel to the websocket
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // Loop to receive messages from websocket and route them
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            // Expect message format: { "target": "peer-id", ... }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(target) = v.get("target").and_then(|t| t.as_str()) {
                    let peers = state.peers.lock().unwrap();
                    if let Some(target_tx) = peers.get(target) {
                        let _ = target_tx.send(text);
                    } else {
                        warn!("Target peer {} not found", target);
                    }
                } else {
                    // Broadcast to all others? Or just ignore?
                    // For "offer"/"answer", we usually need a target.
                    warn!("Message without target received from {}", peer_id);
                }
            }
        }
    }

    // Cleanup
    send_task.abort();
    {
        let mut peers = state.peers.lock().unwrap();
        peers.remove(&peer_id);
    }
    info!("Peer disconnected: {}", peer_id);
}
