mod bindings;
mod handlers;
mod types;

use wit_bindgen::generate;

generate!({
    world: "handler",
    path: "wit",
});

use crate::app::handler::types::ErrorDetails as WitErrorDetails;

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
        // Convert WIT Request to internal Request
        let internal_req = crate::types::Request {
            method: req.method,
            payload: req.payload,
            input_stream: req.input_stream,
            metadata: req.metadata,
            context: crate::types::RequestContext {
                request_id: req.context.request_id,
                service_name: req.context.service_name,
                timestamp: req.context.timestamp,
                transport_info: req.context.transport_info.map(|t| crate::types::TransportInfo {
                    protocol: t.protocol,
                    endpoint: t.endpoint,
                }),
            },
        };

        // Call handler
        let internal_resp = match internal_req.method.as_str() {
            "comments.list" => handlers::handle_list_comments(internal_req),
            "comments.create" => handlers::handle_create_comment(internal_req),
            "files.list" => handlers::handle_list_files(internal_req),
            "files.upload" => handlers::handle_upload_file(internal_req),
            "files.download" => handlers::handle_download_file(internal_req),
            _ => crate::types::Response {
                code: crate::types::codes::NOT_FOUND,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(crate::types::ErrorDetails {
                    message: format!("Unknown method: {}", internal_req.method),
                    code: "METHOD_NOT_FOUND".to_string(),
                    details: None,
                }),
            },
        };

        // Convert internal Response to WIT Response
        Response {
            code: internal_resp.code,
            payload: internal_resp.payload,
            output_stream: internal_resp.output_stream,
            metadata: internal_resp.metadata,
            error: internal_resp.error.map(|e| WitErrorDetails {
                message: e.message,
                code: e.code,
                details: e.details,
            }),
        }
    }

    fn handle_stream_message(ctx: StreamContext, payload: Vec<u8>) -> Response {
        let internal_ctx = crate::types::StreamContext {
            stream_id: ctx.stream_id,
            stream_type: ctx.stream_type,
            metadata: ctx.metadata,
        };

        let internal_resp = match internal_ctx.stream_type.as_str() {
            "chat" => handlers::handle_chat_stream(internal_ctx, payload),
            "notifications" => handlers::handle_notification_stream(internal_ctx, payload),
            _ => crate::types::Response {
                code: crate::types::codes::NOT_FOUND,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(crate::types::ErrorDetails {
                    message: format!("Unknown stream type: {}", internal_ctx.stream_type),
                    code: "STREAM_TYPE_NOT_FOUND".to_string(),
                    details: None,
                }),
            },
        };

        Response {
            code: internal_resp.code,
            payload: internal_resp.payload,
            output_stream: internal_resp.output_stream,
            metadata: internal_resp.metadata,
            error: internal_resp.error.map(|e| WitErrorDetails {
                message: e.message,
                code: e.code,
                details: e.details,
            }),
        }
    }
}

export!(Component);
