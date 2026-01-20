use crate::bindings::{bind_all_capabilities, HostState};
use crate::capabilities::HostCapabilities;
use crate::types::*;
use anyhow::Result;
use wasmtime::*;

pub struct WasmRuntime {
    engine: Engine,
    linker: Linker<HostState>,
    module: Module,
    capabilities: HostCapabilities,
}

impl WasmRuntime {
    pub fn new(capabilities: HostCapabilities, wasm_path: &str) -> Result<Self> {
        let engine = Engine::default();
        let mut linker = Linker::new(&engine);

        // Bind all capabilities
        bind_all_capabilities(&mut linker);

        // Load WASM module
        let module = Module::from_file(&engine, wasm_path)?;

        Ok(WasmRuntime {
            engine,
            linker,
            module,
            capabilities,
        })
    }

    pub fn capabilities(&self) -> &HostCapabilities {
        &self.capabilities
    }

    /// Discover module capabilities
    pub fn discover_capabilities(&self) -> Result<ModuleCapabilities> {
        let mut store = Store::new(
            &self.engine,
            HostState {
                capabilities: self.capabilities.clone(),
            },
        );

        let instance = self.linker.instantiate(&mut store, &self.module)?;

        // Get methods
        let list_methods_fn =
            instance.get_typed_func::<(), (i32, i32)>(&mut store, "list-methods")?;

        let (ptr, len) = list_methods_fn.call(&mut store, ())?;
        let methods_bytes = self.read_from_memory(&mut store, &instance, ptr, len)?;
        let methods: Vec<MethodConfig> = serde_json::from_slice(&methods_bytes)?;

        // Get streams
        let list_streams_fn =
            instance.get_typed_func::<(), (i32, i32)>(&mut store, "list-streams")?;

        let (ptr, len) = list_streams_fn.call(&mut store, ())?;
        let streams_bytes = self.read_from_memory(&mut store, &instance, ptr, len)?;
        let streams: Vec<StreamConfig> = serde_json::from_slice(&streams_bytes)?;

        Ok(ModuleCapabilities { methods, streams })
    }

    /// Handle RPC request
    pub async fn handle_request(&self, request: CanonicalRequest) -> Result<CanonicalResponse> {
        let mut store = Store::new(
            &self.engine,
            HostState {
                capabilities: self.capabilities.clone(),
            },
        );

        let instance = self.linker.instantiate(&mut store, &self.module)?;

        let handle_fn =
            instance.get_typed_func::<(i32, i32), (i32, i32)>(&mut store, "handle-request")?;

        // Serialize request
        let request_bytes = serde_json::to_vec(&request)?;
        let (req_ptr, req_len) = self.write_to_memory(&mut store, &instance, &request_bytes)?;

        // Call WASM
        let (resp_ptr, resp_len) = handle_fn.call(&mut store, (req_ptr, req_len))?;

        // Read response
        let response_bytes = self.read_from_memory(&mut store, &instance, resp_ptr, resp_len)?;
        let response: CanonicalResponse = serde_json::from_slice(&response_bytes)?;

        Ok(response)
    }

    /// Handle stream message
    pub async fn handle_stream_message(
        &self,
        stream_ctx: StreamContext,
        payload: Vec<u8>,
    ) -> Result<CanonicalResponse> {
        let mut store = Store::new(
            &self.engine,
            HostState {
                capabilities: self.capabilities.clone(),
            },
        );

        let instance = self.linker.instantiate(&mut store, &self.module)?;

        let handle_fn = instance.get_typed_func::<(i32, i32, i32, i32), (i32, i32)>(
            &mut store,
            "handle-stream-message",
        )?;

        // Serialize context and payload
        let ctx_bytes = serde_json::to_vec(&stream_ctx)?;
        let (ctx_ptr, ctx_len) = self.write_to_memory(&mut store, &instance, &ctx_bytes)?;
        let (payload_ptr, payload_len) = self.write_to_memory(&mut store, &instance, &payload)?;

        // Call WASM
        let (resp_ptr, resp_len) =
            handle_fn.call(&mut store, (ctx_ptr, ctx_len, payload_ptr, payload_len))?;

        // Read response
        let response_bytes = self.read_from_memory(&mut store, &instance, resp_ptr, resp_len)?;
        let response: CanonicalResponse = serde_json::from_slice(&response_bytes)?;

        Ok(response)
    }

    // Helper to write to WASM memory
    fn write_to_memory(
        &self,
        store: &mut Store<HostState>,
        instance: &Instance,
        data: &[u8],
    ) -> Result<(i32, i32)> {
        let memory = instance
            .get_memory(store, "memory")
            .ok_or_else(|| anyhow::anyhow!("Memory not found"))?;

        let alloc_fn = instance.get_typed_func::<i32, i32>(store, "alloc")?;

        let ptr = alloc_fn.call(store, data.len() as i32)?;
        memory.write(store, ptr as usize, data)?;

        Ok((ptr, data.len() as i32))
    }

    // Helper to read from WASM memory
    fn read_from_memory(
        &self,
        store: &mut Store<HostState>,
        instance: &Instance,
        ptr: i32,
        len: i32,
    ) -> Result<Vec<u8>> {
        let memory = instance
            .get_memory(store, "memory")
            .ok_or_else(|| anyhow::anyhow!("Memory not found"))?;

        let mut buffer = vec![0u8; len as usize];
        memory.read(store, ptr as usize, &mut buffer)?;

        Ok(buffer)
    }
}
