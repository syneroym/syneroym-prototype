# WASM Service - Complete WIT Implementation

A complete, working implementation of a transport-agnostic WASM service using WIT (WebAssembly Interface Types) and the Component Model.

## Quick Start

```bash
# 1. Build everything
chmod +x build.sh
./build.sh

# 2. Run the service
./target/release/host --http-port 3000

# 3. Test it
curl -X POST http://localhost:3000/api/comments/create \
  -H "Content-Type: application/json" \
  -d '{"text": "Hello from WIT!"}'

curl http://localhost:3000/api/comments/list
```

## Project Structure

```
wasm-service/
├── wit/
│   └── world.wit              # WIT interface definitions
├── core/                      # Host runtime with WIT bindings
│   ├── build.rs              # Generate host bindings
│   └── src/
│       ├── runtime.rs        # WasmRuntime + capability implementations
│       └── capabilities/     # DB, files, streams, messaging
├── guest/                     # WASM component (business logic)
│   └── src/
│       ├── bindings.rs       # Generate guest bindings
│       ├── lib.rs            # Main exports
│       └── handlers/         # Business logic
├── transports/
│   ├── http/                 # HTTP/WebSocket adapter
│   └── grpc/                 # gRPC adapter
├── host/                      # Main executable
└── build.sh                   # Build script
```

## How It Works

### 1. WIT Defines the Contract

```wit
// wit/world.wit
interface database {
    execute: func(sql: string, params: list<string>) 
        -> result<query-result, error-details>;
}

world service {
    import database;  // Host provides
    export handler;   // Guest implements
}
```

### 2. Guest Uses Generated Bindings

```rust
// guest/src/handlers/comments.rs
use crate::bindings::wasm::service::database;

// Type-safe, no FFI!
match database::execute("SELECT * FROM comments", &vec![]) {
    Ok(result) => { /* ... */ }
    Err(e) => { /* ... */ }
}
```

### 3. Host Implements Capabilities

```rust
// core/src/runtime.rs

// Wasmtime generates this trait from WIT
impl wasm::service::database::Host for HostState {
    async fn execute(
        &mut self,
        sql: String,
        params: Vec<String>,
    ) -> Result<Result<QueryResult, ErrorDetails>> {
        // Implementation
    }
}
```

## Building from Scratch

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install tools
cargo install wasm-tools
```

### Manual Build

```bash
# 1. Build guest module
cd guest
cargo build --target wasm32-unknown-unknown --release

# 2. Download WASI adapter (first time only)
cd ..
curl -L -o wasi_snapshot_preview1.reactor.wasm \
  https://github.com/bytecodealliance/wasmtime/releases/download/v25.0.0/wasi_snapshot_preview1.reactor.wasm

# 3. Convert to component
wasm-tools component new \
  guest/target/wasm32-unknown-unknown/release/wasm_service_guest.wasm \
  -o handler.wasm \
  --adapt wasi_snapshot_preview1=wasi_snapshot_preview1.reactor.wasm

# 4. Validate
wasm-tools validate handler.wasm --features component-model

# 5. Build host
cargo build --release
```

## API Endpoints

### HTTP (via Axum)

- `POST /api/comments/create` - Create a comment
- `GET /api/comments/list` - List recent comments
- `POST /api/files/upload` - Upload a file
- `GET /api/files/list` - List files
- `GET /api/files/download` - Download a file

### WebSocket

- `ws://localhost:3000/ws/chat` - Chat stream
- `ws://localhost:3000/ws/notifications` - Notifications

### gRPC (Optional)

```bash
# Enable gRPC
./target/release/host --http-port 3000 --grpc-port 50051 --enable-grpc

# Call via grpcurl
grpcurl -plaintext \
  -d '{"method": "comments.list", "payload": "{}"}' \
  localhost:50051 \
  wasm.service.WasmService/Call
```

## Testing

### Create a Comment

```bash
curl -X POST http://localhost:3000/api/comments/create \
  -H "Content-Type: application/json" \
  -d '{"text": "My first comment"}'
```

### List Comments

```bash
curl http://localhost:3000/api/comments/list
```

### Upload a File

```bash
echo "Hello WIT!" > test.txt
curl -X POST http://localhost:3000/api/files/upload \
  -F "file=@test.txt"
```

### List Files

```bash
curl http://localhost:3000/api/files/list
```

### WebSocket Chat

```javascript
const ws = new WebSocket('ws://localhost:3000/ws/chat');

ws.onopen = () => {
  ws.send(JSON.stringify({
    user: 'Alice',
    text: 'Hello everyone!'
  }));
};

ws.onmessage = (event) => {
  console.log('Received:', JSON.parse(event.data));
};
```

## Inspecting the Component

```bash
# View WIT interface
wasm-tools component wit handler.wasm

# Validate component
wasm-tools validate handler.wasm --features component-model

# Print component structure
wasm-tools print handler.wasm | head -50
```

## Key Files Explained

### `wit/world.wit`
Defines the interface contract between host and guest. This is the single source of truth for types and functions.

### `guest/src/bindings.rs`
Generates guest-side bindings via `wit-bindgen`. Provides type-safe Rust APIs for calling host functions.

### `core/build.rs`
Generates host-side bindings via Wasmtime's component bindgen. Creates traits to implement.

### `core/src/runtime.rs`
Implements the generated traits, providing actual functionality for database, files, etc.

## Troubleshooting

### Build Errors

**Error: "wit-bindgen command not found"**
```bash
# wit-bindgen is a dependency, not a command
# Make sure guest/Cargo.toml has wit-bindgen-rt
```

**Error: "adapter not found"**
```bash
# Download the WASI adapter
curl -L -o wasi_snapshot_preview1.reactor.wasm \
  https://github.com/bytecodealliance/wasmtime/releases/download/v25.0.0/wasi_snapshot_preview1.reactor.wasm
```

**Error: "wasmtime version mismatch"**
```bash
# Ensure all workspace members use the same wasmtime version
# Check Cargo.toml workspace.dependencies
```

### Runtime Errors

**Error: "component import not found"**
```bash
# Rebuild with adapter
wasm-tools component new module.wasm -o component.wasm \
  --adapt wasi_snapshot_preview1=wasi_snapshot_preview1.reactor.wasm
```

**Error: "resource table not found"**
```rust
// Make sure HostState implements WasiView correctly
impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx { &mut self.wasi }
    fn table(&mut self) -> &mut ResourceTable { &mut self.table }
}
```

## Development

### Adding a New RPC Method

1. Add to `list_methods()` in `guest/src/lib.rs`
2. Add handler in `guest/src/handlers/`
3. Add route in `handle_request()`
4. Rebuild: `./build.sh`

### Adding a New Capability

1. Define in `wit/world.wit`:
```wit
interface cache {
    get: func(key: string) -> result<option<list<u8>>, error-details>;
    set: func(key: string, value: list<u8>) -> result<_, error-details>;
}

world service {
    import cache;  // Add here
}
```

2. Implement in `core/src/runtime.rs`:
```rust
impl wasm::service::cache::Host for HostState {
    async fn get(&mut self, key: String) 
        -> Result<Result<Option<Vec<u8>>, ErrorDetails>> {
        // Implementation
    }
}
```

3. Use in guest:
```rust
use crate::bindings::wasm::service::cache;

let data = cache::get("key")?;
```

## Performance

WIT bindings add minimal overhead (<5%) compared to manual FFI while providing:
- ✅ Compile-time type checking
- ✅ No manual memory management  
- ✅ Standard interfaces
- ✅ Better tooling

## License

MIT