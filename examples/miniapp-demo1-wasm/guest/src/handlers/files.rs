use crate::bindings::{FileStore, InputStream, OutputStream};
use crate::types::*;

pub fn handle_list_files(_req: Request) -> Response {
    let file_store = FileStore;

    match file_store.list(None) {
        Ok(files) => Response {
            code: codes::SUCCESS,
            payload: serde_json::to_vec(&files).ok(),
            output_stream: None,
            metadata: vec![("content-type".to_string(), "application/json".to_string())],
            error: None,
        },
        Err(e) => Response {
            code: codes::INTERNAL_ERROR,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: Some(ErrorDetails {
                message: format!("File list error: {}", e),
                code: "FILE_ERROR".to_string(),
                details: None,
            }),
        },
    }
}

pub fn handle_upload_file(req: Request) -> Response {
    // Get filename from metadata
    let filename = req
        .metadata
        .iter()
        .find(|(k, _)| k == "content-disposition")
        .and_then(|(_, v)| extract_filename(v))
        .unwrap_or_else(|| "uploaded_file".to_string());

    if let Some(input_stream_id) = req.input_stream {
        let file_store = FileStore;
        let input_stream = InputStream;
        let output_stream = OutputStream;

        // For streaming upload, we would:
        // 1. Read chunks from input stream
        // 2. Write to file via file store
        // For simplification, we'll use small file operations

        // Read from stream
        let mut file_data = Vec::new();
        loop {
            match input_stream.read(&input_stream_id, 8192) {
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
                        error: Some(ErrorDetails {
                            message: format!("Stream read error: {}", e),
                            code: "STREAM_ERROR".to_string(),
                            details: None,
                        }),
                    }
                },
            }
        }

        // Write to file
        match file_store.write_small(&filename, file_data) {
            Ok(()) => Response {
                code: codes::SUCCESS,
                payload: serde_json::to_vec(&serde_json::json!({
                    "filename": filename,
                    "status": "uploaded"
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
                error: Some(ErrorDetails {
                    message: format!("File write error: {}", e),
                    code: "FILE_ERROR".to_string(),
                    details: None,
                }),
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
    // Extract filename from method (e.g., "files.download" with metadata)
    let filename = req
        .metadata
        .iter()
        .find(|(k, _)| k == "filename")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let file_store = FileStore;

    // For small files, read into memory
    match file_store.read_small(&filename) {
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
        Err(_) => Response {
            code: codes::NOT_FOUND,
            payload: None,
            output_stream: None,
            metadata: vec![],
            error: Some(ErrorDetails {
                message: "File not found".to_string(),
                code: "FILE_NOT_FOUND".to_string(),
                details: None,
            }),
        },
    }
}

fn extract_filename(content_disposition: &str) -> Option<String> {
    // Simple filename extraction from content-disposition header
    content_disposition
        .split("filename=")
        .nth(1)
        .map(|s| s.trim_matches('"').to_string())
}
