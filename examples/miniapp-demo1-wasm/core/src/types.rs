use serde::{Deserialize, Serialize};

/// Transport-agnostic request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRequest {
    pub method: String,
    pub payload: Option<Vec<u8>>,
    pub input_stream: Option<String>,
    pub metadata: Vec<(String, String)>,
    pub context: RequestContext,
}

/// Request context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestContext {
    pub request_id: String,
    pub service_name: String,
    pub timestamp: String,
    pub transport_info: Option<TransportInfo>,
}

/// Transport information (for observability only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportInfo {
    pub protocol: String,
    pub endpoint: String,
}

/// Transport-agnostic response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalResponse {
    pub code: u32,
    pub payload: Option<Vec<u8>>,
    pub output_stream: Option<String>,
    pub metadata: Vec<(String, String)>,
    pub error: Option<ErrorDetails>,
}

/// Error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    pub message: String,
    pub code: String,
    pub details: Option<Vec<u8>>,
}

/// Stream context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamContext {
    pub stream_id: String,
    pub stream_type: String,
    pub metadata: Vec<(String, String)>,
}

/// Module capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleCapabilities {
    pub methods: Vec<MethodConfig>,
    pub streams: Vec<StreamConfig>,
}

/// RPC method configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodConfig {
    pub name: String,
    pub request_streaming: bool,
    pub response_streaming: bool,
}

/// Stream type configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_type: String,
    pub bidirectional: bool,
}

/// Stream chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub data: Vec<u8>,
    pub eof: bool,
}

/// Stream info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub id: String,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
    pub filename: Option<String>,
}

/// File info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub content_type: Option<String>,
}

/// Standard response codes (transport-agnostic)
pub mod codes {
    pub const SUCCESS: u32 = 0;
    pub const BAD_REQUEST: u32 = 1;
    pub const NOT_FOUND: u32 = 2;
    pub const INTERNAL_ERROR: u32 = 3;
    pub const UNAUTHORIZED: u32 = 4;
    pub const FORBIDDEN: u32 = 5;
}
