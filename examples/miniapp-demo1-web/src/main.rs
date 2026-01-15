use axum::{
    extract::State,
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use clap::Parser;
use rust_embed::Embed;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Service name to display
    #[arg(long, default_value = "demo1-instance0")]
    service_name: String,

    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[derive(Clone)]
struct AppState {
    service_name: String,
}

#[derive(Embed)]
#[folder = "static/"]
struct Assets;

async fn index_handler(State(state): State<Arc<AppState>>) -> Html<String> {
    Html(format!("<h1>Hello world from {}</h1>", state.service_name))
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data,
            ).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let state = Arc::new(AppState {
        service_name: args.service_name,
    });

    // Build our application with a single route
    let app = Router::new()
        .route("/", get(index_handler))
        .fallback(static_handler)
        .with_state(state);

    // Run it with hyper on localhost
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    println!("listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}