mod bindings;
mod handlers;
mod types;

use types::*;

/// List RPC methods this module exposes
#[no_mangle]
pub extern "C" fn list_methods(result_ptr: *mut i32, result_len: *mut i32) {
    let methods = vec![
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
    ];

    let json = serde_json::to_vec(&methods).unwrap();
    let ptr = bindings::alloc(json.len() as i32);

    unsafe {
        std::ptr::copy_nonoverlapping(json.as_ptr(), ptr, json.len());
        *result_ptr = ptr as i32;
        *result_len = json.len() as i32;
    }

    std::mem::forget(json);
}

/// List stream types this module handles
#[no_mangle]
pub extern "C" fn list_streams(result_ptr: *mut i32, result_len: *mut i32) {
    let streams = vec![
        StreamConfig {
            stream_type: "chat".to_string(),
            bidirectional: true,
        },
        StreamConfig {
            stream_type: "notifications".to_string(),
            bidirectional: false,
        },
    ];

    let json = serde_json::to_vec(&streams).unwrap();
    let ptr = bindings::alloc(json.len() as i32);

    unsafe {
        std::ptr::copy_nonoverlapping(json.as_ptr(), ptr, json.len());
        *result_ptr = ptr as i32;
        *result_len = json.len() as i32;
    }

    std::mem::forget(json);
}

/// Handle RPC request
#[no_mangle]
pub extern "C" fn handle_request(
    req_ptr: i32,
    req_len: i32,
    result_ptr: *mut i32,
    result_len: *mut i32,
) {
    let req_bytes = unsafe { std::slice::from_raw_parts(req_ptr as *const u8, req_len as usize) };

    let request: Request = match serde_json::from_slice(req_bytes) {
        Ok(r) => r,
        Err(e) => {
            let error_response = Response {
                code: codes::BAD_REQUEST,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(ErrorDetails {
                    message: format!("Invalid request: {}", e),
                    code: "INVALID_REQUEST".to_string(),
                    details: None,
                }),
            };

            write_response(error_response, result_ptr, result_len);
            return;
        },
    };

    let response = match request.method.as_str() {
        "comments.list" => handlers::handle_list_comments(request),
        "comments.create" => handlers::handle_create_comment(request),
        "files.list" => handlers::handle_list_files(request),
        "files.upload" => handlers::handle_upload_file(request),
        "files.download" => handlers::handle_download_file(request),
        _ => Response {
            code: codes::NOT_FOUND,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: Some(ErrorDetails {
                message: format!("Unknown method: {}", request.method),
                code: "METHOD_NOT_FOUND".to_string(),
                details: None,
            }),
        },
    };

    write_response(response, result_ptr, result_len);
}

/// Handle stream message
#[no_mangle]
pub extern "C" fn handle_stream_message(
    ctx_ptr: i32,
    ctx_len: i32,
    payload_ptr: i32,
    payload_len: i32,
    result_ptr: *mut i32,
    result_len: *mut i32,
) {
    let ctx_bytes = unsafe { std::slice::from_raw_parts(ctx_ptr as *const u8, ctx_len as usize) };

    let context: StreamContext = match serde_json::from_slice(ctx_bytes) {
        Ok(c) => c,
        Err(e) => {
            let error_response = Response {
                code: codes::BAD_REQUEST,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(ErrorDetails {
                    message: format!("Invalid context: {}", e),
                    code: "INVALID_CONTEXT".to_string(),
                    details: None,
                }),
            };

            write_response(error_response, result_ptr, result_len);
            return;
        },
    };

    let payload = unsafe {
        std::slice::from_raw_parts(payload_ptr as *const u8, payload_len as usize).to_vec()
    };

    let response = match context.stream_type.as_str() {
        "chat" => handlers::handle_chat_stream(context, payload),
        "notifications" => handlers::handle_notification_stream(context, payload),
        _ => Response {
            code: codes::NOT_FOUND,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: Some(ErrorDetails {
                message: format!("Unknown stream type: {}", context.stream_type),
                code: "STREAM_TYPE_NOT_FOUND".to_string(),
                details: None,
            }),
        },
    };

    write_response(response, result_ptr, result_len);
}

fn write_response(response: Response, result_ptr: *mut i32, result_len: *mut i32) {
    let json = serde_json::to_vec(&response).unwrap();
    let ptr = bindings::alloc(json.len() as i32);

    unsafe {
        std::ptr::copy_nonoverlapping(json.as_ptr(), ptr, json.len());
        *result_ptr = ptr as i32;
        *result_len = json.len() as i32;
    }

    std::mem::forget(json);
}
