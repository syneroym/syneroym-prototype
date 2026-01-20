use crate::bindings::{DbQuery, MessageStream};
use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct ChatMessage {
    user: String,
    text: String,
}

pub fn handle_chat_stream(ctx: StreamContext, payload: Vec<u8>) -> Response {
    let message: ChatMessage = match serde_json::from_slice(&payload) {
        Ok(m) => m,
        Err(e) => {
            return Response {
                code: codes::BAD_REQUEST,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(ErrorDetails {
                    message: format!("Invalid message: {}", e),
                    code: "INVALID_MESSAGE".to_string(),
                    details: None,
                }),
            }
        },
    };

    // Validate message
    if message.text.len() > 500 {
        let msg_stream = MessageStream;
        let error = serde_json::json!({
            "error": "Message too long",
            "max_length": 500
        });

        let _ = msg_stream.send(&ctx.stream_id, serde_json::to_vec(&error).unwrap());

        return Response {
            code: codes::BAD_REQUEST,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: None,
        };
    }

    // Store message
    let db = DbQuery;
    let data = serde_json::json!({
        "text": format!("{}: {}", message.user, message.text)
    });

    let message_id = match db.insert("comments", serde_json::to_vec(&data).unwrap()) {
        Ok(id) => id,
        Err(e) => {
            return Response {
                code: codes::INTERNAL_ERROR,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(ErrorDetails {
                    message: format!("Database error: {}", e),
                    code: "DB_ERROR".to_string(),
                    details: None,
                }),
            }
        },
    };

    // Send confirmation to sender
    let msg_stream = MessageStream;
    let confirmation = serde_json::json!({
        "type": "sent",
        "messageId": message_id,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    let _ = msg_stream.send(&ctx.stream_id, serde_json::to_vec(&confirmation).unwrap());

    // Broadcast to all chat participants
    let broadcast = serde_json::json!({
        "type": "new_message",
        "from": message.user,
        "text": message.text,
        "messageId": message_id
    });

    let _ = msg_stream.broadcast("chat", serde_json::to_vec(&broadcast).unwrap());

    Response {
        code: codes::SUCCESS,
        payload: None,
        output_stream: None,
        metadata: vec![],
        error: None,
    }
}

pub fn handle_notification_stream(ctx: StreamContext, payload: Vec<u8>) -> Response {
    // Notifications are typically one-way
    let msg_stream = MessageStream;

    // Echo back acknowledgment
    let ack = serde_json::json!({
        "type": "ack",
        "received": String::from_utf8_lossy(&payload).to_string()
    });

    let _ = msg_stream.send(&ctx.stream_id, serde_json::to_vec(&ack).unwrap());

    Response {
        code: codes::SUCCESS,
        payload: None,
        output_stream: None,
        metadata: vec![],
        error: None,
    }
}
