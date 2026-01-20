#[allow(warnings)]
mod bindings;

mod handlers;

use bindings::exports::wasm::service::handler::Guest;
use bindings::wasm::service::types::*;

struct Component;

impl Guest for Component {
    fn list_methods() -> Vec<MethodConfig> {
        vec![
            MethodConfig {
                name: "comments.list".to_string(),
                request_streaming: false,
                response_streaming: false,
            },
            MethodConfig {
                name: "comments.create".to_string(),
                request_streaming: false,
                response_streaming: false,
            },
            MethodConfig {
                name: "files.list".to_string(),
                request_streaming: false,
                response_streaming: false,
            },
            MethodConfig {
                name: "files.upload".to_string(),
                request_streaming: true,
                response_streaming: false,
            },
            MethodConfig {
                name: "files.download".to_string(),
                request_streaming: false,
                response_streaming: true,
            },
        ]
    }

    fn list_streams() -> Vec<StreamConfig> {
        vec![
            StreamConfig {
                stream_type: "chat".to_string(),
                bidirectional: true,
            },
            StreamConfig {
                stream_type: "notifications".to_string(),
                bidirectional: false,
            },
        ]
    }

    fn handle_request(req: Request) -> Response {
        match req.method.as_str() {
            "comments.list" => handlers::comments::handle_list_comments(req),
            "comments.create" => handlers::comments::handle_create_comment(req),
            "files.list" => handlers::files::handle_list_files(req),
            "files.upload" => handlers::files::handle_upload_file(req),
            "files.download" => handlers::files::handle_download_file(req),
            _ => Response {
                code: codes::NOT_FOUND,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(ErrorDetails {
                    message: format!("Unknown method: {}", req.method),
                    code: "METHOD_NOT_FOUND".to_string(),
                    details: None,
                }),
            },
        }
    }

    fn handle_stream_message(ctx: StreamContext, payload: Vec<u8>) -> Response {
        match ctx.stream_type.as_str() {
            "chat" => handlers::streams::handle_chat_stream(ctx, payload),
            "notifications" => handlers::streams::handle_notification_stream(ctx, payload),
            _ => Response {
                code: codes::NOT_FOUND,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(ErrorDetails {
                    message: format!("Unknown stream type: {}", ctx.stream_type),
                    code: "STREAM_TYPE_NOT_FOUND".to_string(),
                    details: None,
                }),
            },
        }
    }
}

bindings::export!(Component with_types_in bindings);

pub mod codes {
    pub const SUCCESS: u32 = 0;
    pub const BAD_REQUEST: u32 = 1;
    pub const NOT_FOUND: u32 = 2;
    pub const INTERNAL_ERROR: u32 = 3;
    pub const UNAUTHORIZED: u32 = 4;
    pub const FORBIDDEN: u32 = 5;
}
