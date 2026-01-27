use axum::{
    extract::Host,
    response::{Response, IntoResponse},
    http::{HeaderMap, StatusCode, header},
    routing::get,
    Router,
};
use askama::Template;
use std::net::SocketAddr;
use tracing::{info, debug};

pub async fn start(port: u16) -> anyhow::Result<()> {
    info!("Starting PeerNode Web Gateway on port {}", port);

    let app = Router::new()
        .route("/sw.js", get(sw_handler))
        .fallback(index_handler);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("PeerNode Web Gateway listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate;

#[derive(Template)]
#[template(path = "sw.js", escape = "none")]
struct SwTemplate<'a> {
    signaling_server_url: &'a str,
    target_peer_id: &'a str,
}

async fn index_handler(Host(host): Host, headers: HeaderMap) -> Response {
    debug!("Received index request for host: {}", host);

    // Loop Protection
    if headers.contains_key("X-Peer-Proxy") {
         return (
            StatusCode::BAD_GATEWAY,
            "Error: Request reached gateway server. Service Worker failed to intercept.",
        ).into_response();
    }

    let template = IndexTemplate;

    match template.render() {
        Ok(content) => {
            Response::builder()
                .header(header::CONTENT_TYPE, "text/html")
                .body(content.into())
                .unwrap()
        },
        Err(e) => {
            tracing::error!("Index template rendering failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}

async fn sw_handler() -> Response {
    debug!("Received SW request");

    let template = SwTemplate {
        signaling_server_url: "ws://localhost:8000/ws",
        target_peer_id: "host-node",
    };

    match template.render() {
        Ok(content) => {
            Response::builder()
                .header(header::CONTENT_TYPE, "application/javascript")
                .header("Service-Worker-Allowed", "/") 
                .body(content.into())
                .unwrap()
        },
        Err(e) => {
            tracing::error!("SW template rendering failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}