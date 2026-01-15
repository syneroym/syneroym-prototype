use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Json, State,
    },
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use clap::Parser;
use rusqlite::{params, Connection};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Service name to display
    #[arg(long, default_value = "demo1-instance0")]
    service_name: String,

    /// Port to listen on
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Database URL (file path for rusqlite)
    #[arg(long, default_value = "db/comments.db")]
    database_url: String,
}

#[derive(Clone)]
struct AppState {
    service_name: String,
    // Connection is not Sync, so we need Mutex.
    // We use std::sync::Mutex because we are inside spawn_blocking mostly,
    // and rusqlite is blocking.
    conn: Arc<Mutex<Connection>>,
    tx: broadcast::Sender<String>,
}

#[derive(Embed)]
#[folder = "static/"]
struct Assets;

async fn index_handler(State(state): State<Arc<AppState>>) -> Html<String> {
    Html(format!(
        "<h1>Hello world from {}</h1><p><a href='/comments'>Go to Comments</a></p>",
        state.service_name
    ))
}

async fn comments_page_handler() -> impl IntoResponse {
    match Assets::get("dist/index.html") {
        Some(content) => Html(String::from_utf8_lossy(&content.data).to_string()).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            "Comments page not found. Did you build the client?",
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CreateComment {
    text: String,
}

#[derive(Serialize)]
struct Comment {
    id: i64,
    text: String,
}

async fn get_recent_comments(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, text FROM comments ORDER BY id DESC LIMIT 5")
            .map_err(|e| e.to_string())?;

        let comments_iter = stmt
            .query_map([], |row| {
                Ok(Comment {
                    id: row.get(0)?,
                    text: row.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?;

        let mut comments = Vec::new();
        for comment in comments_iter {
            comments.push(comment.map_err(|e| e.to_string())?);
        }
        Ok::<_, String>(comments)
    })
    .await;

    match result {
        Ok(Ok(comments)) => Json(comments).into_response(),
        Ok(Err(e)) => {
            eprintln!("Database query error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        },
        Err(e) => {
            eprintln!("Join error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        },
    }
}

async fn save_comment(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateComment>,
) -> impl IntoResponse {
    let state_for_db = state.clone();
    let text = payload.text.clone();

    // Offload blocking DB operation to a thread pool
    let result = tokio::task::spawn_blocking(move || {
        let conn = state_for_db.conn.lock().unwrap();
        conn.execute("INSERT INTO comments (text) VALUES (?)", params![text])
    })
    .await;

    match result {
        Ok(Ok(_)) => {
            let _ = state.tx.send(chrono::Utc::now().to_rfc3339());
            StatusCode::CREATED
        },
        Ok(Err(e)) => {
            eprintln!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        },
        Err(e) => {
            eprintln!("Join error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        },
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            Some(msg) = socket.recv() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        println!("[{}] Received: {}", chrono::Utc::now(), text);
                    }
                    Ok(Message::Close(_)) | Err(_) => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        },
        None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let db_path = args.database_url.trim_start_matches("sqlite:");
    // rusqlite creates the file if it doesn't exist by default with Connection::open
    let conn = Connection::open(db_path).expect("Failed to connect to database");

    // Run migration
    conn.execute(
        "CREATE TABLE IF NOT EXISTS comments (id INTEGER PRIMARY KEY, text TEXT)",
        [],
    )
    .expect("Failed to create table");

    let (tx, _rx) = broadcast::channel(100);

    let state = Arc::new(AppState {
        service_name: args.service_name,
        conn: Arc::new(Mutex::new(conn)),
        tx,
    });

    // Build our application
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/comments", get(comments_page_handler))
        .route("/api/comments", post(save_comment).get(get_recent_comments))
        .route("/ws", get(websocket_handler))
        .fallback(static_handler)
        .with_state(state);

    // Run it with hyper on localhost
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    println!("listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
