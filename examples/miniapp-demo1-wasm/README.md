# WASM Service - Transport-Agnostic Architecture

A complete implementation of a transport-agnostic WASM-based service that supports both HTTP and gRPC simultaneously, with all business logic contained in a WASM module.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│          Transport Layer (HTTP/gRPC)                         │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│          WASM Runtime (Transport Agnostic)                   │
│  • Stream Manager                                            │
│  • Capability Providers (DB, Files, Messaging)               │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│          WASM Module (Pure Business Logic)                   │
│  • No knowledge of HTTP/gRPC/WebSocket                       │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

```
wasm-service/
├── core/                   # Core WASM runtime & capabilities
├── transports/
│   ├── http/              # HTTP/WebSocket adapter
│   └── grpc/              # gRPC adapter
├── host/                  # Main host application
├── guest/                 # WASM business logic module
└── static/                # Static assets
```

## Building

### 1. Build the WASM guest module

```bash
cd guest
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/wasm_service_guest.wasm ../handler.wasm
```

### 2. Build the host

```bash
cargo build --release
```

## Running

### HTTP only

```bash
./target/release/host \
  --service-name my-service \
  --http-port 3000 \
  --data-dir ./data \
  --wasm-path ./handler.wasm
```

### HTTP + gRPC

```bash
./target/release/host \
  --service-name my-service \
  --http-port 3000 \
  --grpc-port 50051 \
  --enable-grpc \
  --data-dir ./data \
  --wasm-path ./handler.wasm
```

## API Endpoints

The WASM module exposes the following RPC methods:

### HTTP Endpoints

- `POST /api/comments/list` - List recent comments
- `POST /api/comments/create` - Create a new comment
- `POST /api/files/list` - List uploaded files
- `POST /api/files/upload` - Upload a file (multipart)
- `POST /api/files/download` - Download a file

### WebSocket Endpoints

- `ws://localhost:3000/ws/chat` - Bidirectional chat
- `ws://localhost:3000/ws/notifications` - One-way notifications

### gRPC Methods

All HTTP endpoints are also available via gRPC using the `WasmService.Call` method.

## Example Usage

### Create a comment (HTTP)

```bash
curl -X POST http://localhost:3000/api/comments/create \
  -H "Content-Type: application/json" \
  -d '{"text": "Hello from HTTP!"}'
```

### Create a comment (gRPC)

```bash
grpcurl -plaintext \
  -d '{"method": "comments.create", "payload": "{\"text\":\"Hello from gRPC!\"}"}' \
  localhost:50051 \
  wasm.service.WasmService/Call
```

### WebSocket chat

```javascript
const ws = new WebSocket('ws://localhost:3000/ws/chat');

ws.onopen = () => {
  ws.send(JSON.stringify({
    user: 'Alice',
    text: 'Hello everyone!'
  }));
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Received:', data);
};
```

### Upload a file

```bash
curl -X POST http://localhost:3000/api/files/upload \
  -F "file=@myfile.txt"
```

## Key Features

1. **Transport Agnostic**: WASM module has no knowledge of HTTP, gRPC, or WebSockets
2. **Capability-Based**: WASM accesses resources (DB, files, messaging) through host-provided capabilities
3. **Streaming Support**: Handles large file uploads/downloads efficiently
4. **Bidirectional Messaging**: WebSocket and gRPC streaming support
5. **Hot Reloadable**: Update business logic without restarting the host (future feature)
6. **Type-Safe**: Strongly typed interfaces between host and guest

## Extending

### Adding a new RPC method

1. Add method config to `guest/src/lib.rs` in `list_methods()`
2. Implement handler in `guest/src/handlers/`
3. Add routing in `handle_request()` function
4. Rebuild WASM module

No changes needed to host code!

### Adding a new transport

1. Create new adapter in `transports/`
2. Implement conversion between transport and `CanonicalRequest`/`CanonicalResponse`
3. Register in `host/src/main.rs`

## Development

### Running tests

```bash
cargo test --workspace
```

### Viewing logs

```bash
RUST_LOG=debug ./target/release/host ...
```

## License

MIT
