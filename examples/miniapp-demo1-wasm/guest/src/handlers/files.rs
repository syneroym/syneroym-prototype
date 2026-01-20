use crate::bindings::wasm::service::{files, streams, types::*};
use crate::codes;

pub fn handle_list_files(_req: Request) -> Response {
    match files::list(None) {
        Ok(file_list) => Response {
            code: codes::SUCCESS,
            payload: serde_json::to_vec(&file_list).ok(),
            output_stream: None,
            metadata: vec![("content-type".to_string(), "application/json".to_string())],
            error: None,
        },
        Err(e) => Response {
            code: codes::INTERNAL_ERROR,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: Some(e),
        },
    }
}

pub fn handle_upload_file(req: Request) -> Response {
    let filename = req
        .metadata
        .iter()
        .find(|(k, _)| k == "content-disposition")
        .and_then(|(_, v)| extract_filename(v))
        .unwrap_or_else(|| "uploaded_file".to_string());

    if let Some(input_stream_id) = req.input_stream {
        let mut file_data = Vec::new();

        loop {
            match streams::read(&input_stream_id, 8192) {
                Ok(chunk) => {
                    file_data.extend_from_slice(&chunk.data);
                    if chunk.eof {
                        break;
                    }
                },
                Err(e) => {
                    return Response {
                        code: codes::INTERNAL_ERROR,
                        payload: None,
                        output_stream: None,
                        metadata: vec![],
                        error: Some(e),
                    }
                },
            }
        }

        let _ = streams::close_input(&input_stream_id);

        match files::write_small(&filename, &file_data) {
            Ok(()) => Response {
                code: codes::SUCCESS,
                payload: serde_json::to_vec(&serde_json::json!({
                    "filename": filename,
                    "status": "uploaded",
                    "size": file_data.len()
                }))
                .ok(),
                output_stream: None,
                metadata: vec![("content-type".to_string(), "application/json".to_string())],
                error: None,
            },
            Err(e) => Response {
                code: codes::INTERNAL_ERROR,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(e),
            },
        }
    } else {
        Response {
            code: codes::BAD_REQUEST,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: Some(ErrorDetails {
                message: "No input stream provided".to_string(),
                code: "MISSING_STREAM".to_string(),
                details: None,
            }),
        }
    }
}

pub fn handle_download_file(req: Request) -> Response {
    let filename = req
        .metadata
        .iter()
        .find(|(k, _)| k == "filename")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "unknown".to_string());

    match files::read_small(&filename) {
        Ok(data) => {
            let content_type = mime_guess::from_path(&filename)
                .first()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());

            Response {
                code: codes::SUCCESS,
                payload: Some(data),
                output_stream: None,
                metadata: vec![
                    ("content-type".to_string(), content_type),
                    (
                        "content-disposition".to_string(),
                        format!("attachment; filename=\"{}\"", filename),
                    ),
                ],
                error: None,
            }
        },
        Err(e) => {
            let code = if e.code == "FILE_NOT_FOUND" {
                codes::NOT_FOUND
            } else {
                codes::INTERNAL_ERROR
            };

            Response {
                code,
                payload: None,
                output_stream: None,
                metadata: vec![],
                error: Some(e),
            }
        },
    }
}

fn extract_filename(content_disposition: &str) -> Option<String> {
    content_disposition
        .split("filename=")
        .nth(1)
        .map(|s| s.trim_matches('"').to_string())
}
