use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub payload: Option<Vec<u8>>,
    pub input_stream: Option<String>,
    pub metadata: Vec<(String, String)>,
    pub context: RequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub request_id: String,
    pub service_name: String,
    pub timestamp: String,
    pub transport_info: Option<TransportInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportInfo {
    pub protocol: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub code: u32,
    pub payload: Option<Vec<u8>>,
    pub output_stream: Option<String>,
    pub metadata: Vec<(String, String)>,
    pub error: Option<ErrorDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    pub message: String,
    pub code: String,
    pub details: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamContext {
    pub stream_id: String,
    pub stream_type: String,
    pub metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodConfig {
    pub name: String,
    pub request_streaming: bool,
    pub response_streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_type: String,
    pub bidirectional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub data: Vec<u8>,
    pub eof: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub content_type: Option<String>,
}

// Response codes
pub mod codes {
    pub const SUCCESS: u32 = 0;
    pub const BAD_REQUEST: u32 = 1;
    pub const NOT_FOUND: u32 = 2;
    pub const INTERNAL_ERROR: u32 = 3;
    pub const UNAUTHORIZED: u32 = 4;
    pub const FORBIDDEN: u32 = 5;
}
