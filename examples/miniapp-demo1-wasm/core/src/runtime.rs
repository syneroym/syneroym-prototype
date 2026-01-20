use crate::capabilities::HostCapabilities;
use crate::types::*;
use anyhow::Result;
use async_trait::async_trait;
use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

// Generate bindings from WIT
wasmtime::component::bindgen!({
    world: "service",
    path: "../wit",
    async: true,
    tracing: true,
});

pub struct HostState {
    pub capabilities: HostCapabilities,
    pub wasi: WasiCtx,
    pub table: ResourceTable,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

pub struct WasmRuntime {
    engine: Engine,
    linker: Linker<HostState>,
    component: Component,
    capabilities: HostCapabilities,
}

impl WasmRuntime {
    pub fn new(capabilities: HostCapabilities, wasm_path: &str) -> Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;
        let mut linker = Linker::new(&engine);

        wasmtime_wasi::add_to_linker_async(&mut linker)?;
        Service::add_to_linker(&mut linker, |s| s)?;

        let component = Component::from_file(&engine, wasm_path)?;

        Ok(WasmRuntime {
            engine,
            linker,
            component,
            capabilities,
        })
    }

    pub fn capabilities(&self) -> &HostCapabilities {
        &self.capabilities
    }

    fn create_store(&self) -> Store<HostState> {
        let wasi = WasiCtxBuilder::new().inherit_stdio().inherit_env().build();

        Store::new(
            &self.engine,
            HostState {
                capabilities: self.capabilities.clone(),
                wasi,
                table: ResourceTable::new(),
            },
        )
    }

    pub async fn discover_capabilities(&self) -> Result<ModuleCapabilities> {
        let mut store = self.create_store();
        let bindings =
            Service::instantiate_async(&mut store, &self.component, &self.linker).await?;

        let methods = bindings
            .wasm_service_handler()
            .call_list_methods(&mut store)
            .await?;

        let streams = bindings
            .wasm_service_handler()
            .call_list_streams(&mut store)
            .await?;

        Ok(ModuleCapabilities {
            methods: methods
                .into_iter()
                .map(|m| MethodConfig {
                    name: m.name,
                    request_streaming: m.request_streaming,
                    response_streaming: m.response_streaming,
                })
                .collect(),
            streams: streams
                .into_iter()
                .map(|s| StreamConfig {
                    stream_type: s.stream_type,
                    bidirectional: s.bidirectional,
                })
                .collect(),
        })
    }

    pub async fn handle_request(&self, request: CanonicalRequest) -> Result<CanonicalResponse> {
        let mut store = self.create_store();
        let bindings =
            Service::instantiate_async(&mut store, &self.component, &self.linker).await?;

        let wit_request = wasm::service::types::Request {
            method: request.method,
            payload: request.payload,
            input_stream: request.input_stream,
            metadata: request.metadata,
            context: wasm::service::types::RequestContext {
                request_id: request.context.request_id,
                service_name: request.context.service_name,
                timestamp: request.context.timestamp,
                transport_info: request.context.transport_info.map(|ti| {
                    wasm::service::types::TransportInfo {
                        protocol: ti.protocol,
                        endpoint: ti.endpoint,
                    }
                }),
            },
        };

        let wit_response = bindings
            .wasm_service_handler()
            .call_handle_request(&mut store, &wit_request)
            .await?;

        Ok(CanonicalResponse {
            code: wit_response.code,
            payload: wit_response.payload,
            output_stream: wit_response.output_stream,
            metadata: wit_response.metadata,
            error: wit_response.error.map(|e| ErrorDetails {
                message: e.message,
                code: e.code,
                details: e.details,
            }),
        })
    }

    pub async fn handle_stream_message(
        &self,
        stream_ctx: StreamContext,
        payload: Vec<u8>,
    ) -> Result<CanonicalResponse> {
        let mut store = self.create_store();
        let bindings =
            Service::instantiate_async(&mut store, &self.component, &self.linker).await?;

        let wit_ctx = wasm::service::types::StreamContext {
            stream_id: stream_ctx.stream_id,
            stream_type: stream_ctx.stream_type,
            metadata: stream_ctx.metadata,
        };

        let wit_response = bindings
            .wasm_service_handler()
            .call_handle_stream_message(&mut store, &wit_ctx, &payload)
            .await?;

        Ok(CanonicalResponse {
            code: wit_response.code,
            payload: wit_response.payload,
            output_stream: wit_response.output_stream,
            metadata: wit_response.metadata,
            error: wit_response.error.map(|e| ErrorDetails {
                message: e.message,
                code: e.code,
                details: e.details,
            }),
        })
    }
}

// Implement host-side capability interfaces
impl wasm::service::types::Host for HostState {}

#[async_trait]
impl wasm::service::database::Host for HostState {
    async fn execute(
        &mut self,
        sql: String,
        params: Vec<String>,
    ) -> std::result::Result<wasm::service::database::QueryResult, wasm::service::types::ErrorDetails>
    {
        use crate::capabilities::db::DbOperations;

        let conn = self.capabilities.db.lock().unwrap();
        match DbOperations::execute(&conn, &sql, params) {
            Ok(rows) => Ok(wasm::service::database::QueryResult {
                rows: rows
                    .into_iter()
                    .map(|row| wasm::service::database::Row { columns: row })
                    .collect(),
            }),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "DB_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn insert(
        &mut self,
        table: String,
        data: Vec<u8>,
    ) -> std::result::Result<u64, wasm::service::types::ErrorDetails> {
        use crate::capabilities::db::DbOperations;

        let conn = self.capabilities.db.lock().unwrap();
        match DbOperations::insert(&conn, &table, data) {
            Ok(id) => Ok(id),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "DB_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn update(
        &mut self,
        table: String,
        id: u64,
        data: Vec<u8>,
    ) -> std::result::Result<bool, wasm::service::types::ErrorDetails> {
        use crate::capabilities::db::DbOperations;

        let conn = self.capabilities.db.lock().unwrap();
        match DbOperations::update(&conn, &table, id, data) {
            Ok(success) => Ok(success),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "DB_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn delete(
        &mut self,
        table: String,
        id: u64,
    ) -> std::result::Result<bool, wasm::service::types::ErrorDetails> {
        use crate::capabilities::db::DbOperations;

        let conn = self.capabilities.db.lock().unwrap();
        match DbOperations::delete(&conn, &table, id) {
            Ok(success) => Ok(success),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "DB_ERROR".to_string(),
                details: None,
            }),
        }
    }
}

#[async_trait]
impl wasm::service::files::Host for HostState {
    async fn list(
        &mut self,
        prefix: Option<String>,
    ) -> std::result::Result<Vec<wasm::service::files::FileInfo>, wasm::service::types::ErrorDetails>
    {
        use crate::capabilities::files::FileOperations;

        match FileOperations::list_files(&self.capabilities.data_dir, prefix).await {
            Ok(files) => Ok(files
                .into_iter()
                .map(|f| wasm::service::files::FileInfo {
                    name: f.name,
                    size: f.size,
                    content_type: f
                        .content_type
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                })
                .collect()),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "FILE_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn read_small(
        &mut self,
        path: String,
    ) -> std::result::Result<Vec<u8>, wasm::service::types::ErrorDetails> {
        use crate::capabilities::files::FileOperations;

        match FileOperations::read_small(&self.capabilities.data_dir, &path).await {
            Ok(data) => Ok(data),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: if e.to_string().contains("not found") {
                    "FILE_NOT_FOUND"
                } else {
                    "FILE_ERROR"
                }
                .to_string(),
                details: None,
            }),
        }
    }

    async fn write_small(
        &mut self,
        path: String,
        data: Vec<u8>,
    ) -> std::result::Result<(), wasm::service::types::ErrorDetails> {
        use crate::capabilities::files::FileOperations;

        match FileOperations::write_small(&self.capabilities.data_dir, &path, data).await {
            Ok(()) => Ok(()),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "FILE_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn delete(
        &mut self,
        path: String,
    ) -> std::result::Result<bool, wasm::service::types::ErrorDetails> {
        use crate::capabilities::files::FileOperations;

        match FileOperations::delete(&self.capabilities.data_dir, &path).await {
            Ok(success) => Ok(success),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "FILE_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn exists(&mut self, path: String) -> bool {
        use crate::capabilities::files::FileOperations;
        FileOperations::exists(&self.capabilities.data_dir, &path)
    }

    async fn get_info(
        &mut self,
        path: String,
    ) -> std::result::Result<wasm::service::files::FileInfo, wasm::service::types::ErrorDetails>
    {
        use crate::capabilities::files::FileOperations;

        match FileOperations::get_info(&self.capabilities.data_dir, &path).await {
            Ok(info) => Ok(wasm::service::files::FileInfo {
                name: info.name,
                size: info.size,
                content_type: info
                    .content_type
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
            }),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "FILE_ERROR".to_string(),
                details: None,
            }),
        }
    }
}

#[async_trait]
impl wasm::service::streams::Host for HostState {
    async fn read(
        &mut self,
        stream_id: String,
        max_bytes: u32,
    ) -> std::result::Result<wasm::service::streams::StreamChunk, wasm::service::types::ErrorDetails>
    {
        match self
            .capabilities
            .stream_manager
            .read_chunk(&stream_id, max_bytes as usize)
            .await
        {
            Ok(chunk) => Ok(wasm::service::streams::StreamChunk {
                data: chunk.data,
                eof: chunk.eof,
            }),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "STREAM_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn write(
        &mut self,
        stream_id: String,
        data: Vec<u8>,
    ) -> std::result::Result<u32, wasm::service::types::ErrorDetails> {
        match self
            .capabilities
            .stream_manager
            .write_chunk(&stream_id, data)
            .await
        {
            Ok(written) => Ok(written as u32),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "STREAM_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn close_input(
        &mut self,
        stream_id: String,
    ) -> std::result::Result<(), wasm::service::types::ErrorDetails> {
        self.capabilities
            .stream_manager
            .close_input_stream(&stream_id);
        Ok(())
    }

    async fn finish_output(
        &mut self,
        stream_id: String,
    ) -> std::result::Result<(), wasm::service::types::ErrorDetails> {
        match self
            .capabilities
            .stream_manager
            .finish_output_stream(&stream_id)
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "STREAM_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn get_info(
        &mut self,
        stream_id: String,
    ) -> std::result::Result<wasm::service::streams::StreamInfo, wasm::service::types::ErrorDetails>
    {
        match self.capabilities.stream_manager.get_info(&stream_id) {
            Some(info) => Ok(wasm::service::streams::StreamInfo {
                id: info.id,
                content_type: info
                    .content_type
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
                content_length: info.content_length,
                filename: info.filename,
            }),
            None => Err(wasm::service::types::ErrorDetails {
                message: "Stream not found".to_string(),
                code: "STREAM_NOT_FOUND".to_string(),
                details: None,
            }),
        }
    }
}

#[async_trait]
impl wasm::service::messaging::Host for HostState {
    async fn send(
        &mut self,
        stream_id: String,
        payload: Vec<u8>,
    ) -> std::result::Result<(), wasm::service::types::ErrorDetails> {
        match self.capabilities.message_streams.send(&stream_id, payload) {
            Ok(()) => Ok(()),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "MESSAGING_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn broadcast(
        &mut self,
        stream_type: String,
        payload: Vec<u8>,
    ) -> std::result::Result<u32, wasm::service::types::ErrorDetails> {
        match self
            .capabilities
            .message_streams
            .broadcast(&stream_type, payload)
        {
            Ok(count) => Ok(count as u32),
            Err(e) => Err(wasm::service::types::ErrorDetails {
                message: e.to_string(),
                code: "MESSAGING_ERROR".to_string(),
                details: None,
            }),
        }
    }

    async fn close(
        &mut self,
        _stream_id: String,
    ) -> std::result::Result<(), wasm::service::types::ErrorDetails> {
        Ok(())
    }
}
