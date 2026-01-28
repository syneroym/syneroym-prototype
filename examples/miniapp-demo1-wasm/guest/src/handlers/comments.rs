use crate::bindings::wasm::service::{database, messaging, types::*};
use crate::codes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateComment {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Comment {
    pub id: i64,
    pub text: String,
}

pub fn handle_list_comments(_req: Request) -> Response {
    match database::execute(
        "SELECT id, text FROM comments ORDER BY id DESC LIMIT 5",
        &[],
    ) {
        Ok(result) => {
            let comments: Vec<Comment> = result
                .rows
                .iter()
                .filter_map(|row| {
                    if row.columns.len() >= 2 {
                        let id = String::from_utf8_lossy(&row.columns[0])
                            .parse::<i64>()
                            .unwrap_or(0);
                        let text = String::from_utf8_lossy(&row.columns[1]).to_string();
                        Some(Comment { id, text })
                    } else {
                        None
                    }
                })
                .collect();

            Response {
                code: codes::SUCCESS,
                payload: serde_json::to_vec(&comments).ok(),
                output_stream: None,
                metadata: vec![("content-type".to_string(), "application/json".to_string())],
                error: None,
            }
        }
        Err(e) => Response {
            code: codes::INTERNAL_ERROR,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: Some(e),
        },
    }
}

pub fn handle_create_comment(req: Request) -> Response {
    let payload = match req.payload {
        Some(p) => p,
        None => {
            return Response {
                code: codes::BAD_REQUEST,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(ErrorDetails {
                    message: "No payload provided".to_string(),
                    code: "MISSING_PAYLOAD".to_string(),
                    details: None,
                }),
            };
        }
    };

    let comment: CreateComment = match serde_json::from_slice(&payload) {
        Ok(c) => c,
        Err(e) => {
            return Response {
                code: codes::BAD_REQUEST,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(ErrorDetails {
                    message: format!("Invalid JSON: {}", e),
                    code: "INVALID_JSON".to_string(),
                    details: None,
                }),
            };
        }
    };

    let data = serde_json::json!({ "text": comment.text });

    match database::insert("comments", &serde_json::to_vec(&data).unwrap()) {
        Ok(id) => {
            let notification = serde_json::json!({
                "type": "comment_created",
                "id": id,
                "timestamp": req.context.timestamp
            });

            let _ =
                messaging::broadcast("notifications", &serde_json::to_vec(&notification).unwrap());

            Response {
                code: codes::SUCCESS,
                payload: serde_json::to_vec(&serde_json::json!({ "id": id })).ok(),
                output_stream: None,
                metadata: vec![("content-type".to_string(), "application/json".to_string())],
                error: None,
            }
        }
        Err(e) => Response {
            code: codes::INTERNAL_ERROR,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: Some(e),
        },
    }
}
