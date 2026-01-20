#!/bin/bash
set -e

echo "Building WASM Service with WIT/Component Model..."

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo not found. Please install Rust."
    exit 1
fi

# Check if wasm32-unknown-unknown target is installed
if ! rustup target list | grep -q "wasm32-unknown-unknown (installed)"; then
    echo "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# Check if wasm-tools is installed
if ! command -v wasm-tools &> /dev/null; then
    echo "wasm-tools not found. Installing..."
    cargo install wasm-tools
fi

# Check if wasm component adapter exists
ADAPTER_FILE="wasi_snapshot_preview1.reactor.wasm"
if [ ! -f "$ADAPTER_FILE" ]; then
    echo "WASI adapter not found. Downloading..."
    curl -L -o "$ADAPTER_FILE" \
        "https://github.com/bytecodealliance/wasmtime/releases/download/v25.0.0/wasi_snapshot_preview1.reactor.wasm"
fi

echo ""
echo "Step 1: Building guest WASM module..."
cd guest
cargo build --target wasm32-unknown-unknown --release
cd ..

echo ""
echo "Step 2: Converting module to component..."
wasm-tools component new \
    ../../target/wasm32-unknown-unknown/release/wasm_service_guest.wasm \
    -o handler.wasm \
    --adapt "wasi_snapshot_preview1=$ADAPTER_FILE"

echo ""
echo "Step 3: Verifying component..."
wasm-tools validate handler.wasm --features component-model

echo ""
echo "Step 4: Building host application..."
cargo build --release -p wasm-service-host

# Create data directory
mkdir -p data

echo ""
echo "✅ Build complete!"
echo ""
echo "Component info:"
wasm-tools component wit handler.wasm
echo ""
echo "To run HTTP only:"
echo "  ./target/release/host --service-name my-service --http-port 3000"
echo ""
echo "To run HTTP + gRPC:"
echo "  ./target/release/host --service-name my-service --http-port 3000 --grpc-port 50051 --enable-grpc"
echo ""