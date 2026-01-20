use clap::Parser;
use rusqlite::Connection;
use std::net::SocketAddr;
use std::sync::Arc;
use wasm_service_core::{HostCapabilities, WasmRuntime};
#[cfg(feature = "grpc")]
use wasm_service_grpc::GrpcTransport;
use wasm_service_http::HttpTransport;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Service name
    #[arg(long, default_value = "demo-service")]
    service_name: String,

    /// HTTP port
    #[arg(long, default_value_t = 3000)]
    http_port: u16,

    /// gRPC port
    #[arg(long, default_value_t = 50051)]
    grpc_port: u16,

    /// Data directory
    #[arg(long, default_value = "data")]
    data_dir: String,

    /// WASM module path
    #[arg(long, default_value = "handler.wasm")]
    wasm_path: String,

    /// Enable gRPC
    #[arg(long, default_value_t = false)]
    enable_grpc: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    // Create data directory
    std::fs::create_dir_all(&args.data_dir)?;

    // Initialize database
    let db_path = std::path::Path::new(&args.data_dir).join("service.db");
    let conn = Connection::open(&db_path)?;

    // Run migrations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS comments (
            id INTEGER PRIMARY KEY,
            text TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    tracing::info!("Database initialized at {:?}", db_path);

    // Create host capabilities
    let capabilities = HostCapabilities::new(&args.data_dir, conn);

    // Create WASM runtime
    let runtime = Arc::new(WasmRuntime::new(capabilities, &args.wasm_path)?);

    tracing::info!("WASM module loaded from {}", args.wasm_path);

    // Discover module capabilities
    let module_caps = runtime.discover_capabilities().await?;
    tracing::info!(
        "Module exposes {} methods and {} stream types",
        module_caps.methods.len(),
        module_caps.streams.len()
    );

    for method in &module_caps.methods {
        tracing::info!(
            "  Method: {} (request_streaming={}, response_streaming={})",
            method.name,
            method.request_streaming,
            method.response_streaming
        );
    }

    for stream in &module_caps.streams {
        tracing::info!(
            "  Stream: {} (bidirectional={})",
            stream.stream_type,
            stream.bidirectional
        );
    }

    // Start HTTP transport
    let http_transport = HttpTransport::new(runtime.clone(), args.service_name.clone());
    let http_router = http_transport.build_router().await;
    let http_addr = SocketAddr::from(([127, 0, 0, 1], args.http_port));

    let http_server = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();
        tracing::info!("HTTP server listening on http://{}", http_addr);
        axum::serve(listener, http_router).await.unwrap();
    });

    // Start gRPC transport if enabled
    let grpc_server: Option<tokio::task::JoinHandle<()>> = if args.enable_grpc {
        #[cfg(feature = "grpc")]
        {
            let grpc_transport = GrpcTransport::new(runtime.clone(), args.service_name.clone());
            let grpc_addr = SocketAddr::from(([127, 0, 0, 1], args.grpc_port));

            Some(tokio::spawn(async move {
                tracing::info!("gRPC server listening on {}", grpc_addr);
                grpc_transport.serve(grpc_addr).await.unwrap();
            }))
        }
        #[cfg(not(feature = "grpc"))]
        {
            tracing::warn!("gRPC enabled in args but 'grpc' feature is disabled. Ignoring.");
            None
        }
    } else {
        None
    };

    tracing::info!("Service '{}' started successfully", args.service_name);

    // Wait for servers
    if let Some(grpc) = grpc_server {
        tokio::try_join!(http_server, grpc)?;
    } else {
        http_server.await?;
    }

    Ok(())
}
