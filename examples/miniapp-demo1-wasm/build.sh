#!/bin/bash
set -e

echo "Building WASM Service..."

# Check if rust target is installed
if ! rustup target list | grep -q "wasm32-unknown-unknown (installed)"; then
    echo "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# Build guest WASM module
echo "Building guest WASM module..."
cd guest
cargo build --target wasm32-unknown-unknown --release
cd ..

# Copy WASM module to root
echo "Copying WASM module..."
cp guest/target/wasm32-unknown-unknown/release/wasm_service_guest.wasm handler.wasm

# Build host
echo "Building host application..."
cargo build --release

# Create data directory
mkdir -p data

echo ""
echo "Build complete!"
echo ""
echo "To run HTTP only:"
echo "  ./target/release/host --service-name my-service --http-port 3000"
echo ""
echo "To run HTTP + gRPC:"
echo "  ./target/release/host --service-name my-service --http-port 3000 --grpc-port 50051 --enable-grpc"
echo ""