mod err;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{cell::Cell, net::SocketAddr};
use tokio::sync::Mutex;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::{any, get},
    Router,
};

use err::PhonexError;
use lazy_static::lazy_static;
use tracing_spanned::SpanErr;

use webrtc::{
    ice_transport::ice_candidate::RTCIceCandidate,
    peer_connection::sdp::session_description::RTCSessionDescription,
};

lazy_static! {
    pub static ref PENDING_CANDIDATES: Arc<Mutex<Vec<RTCIceCandidate>>> =
        Arc::new(Mutex::new(vec![]));
}

#[tokio::main]
async fn main() -> Result<(), SpanErr<PhonexError>> {
    let app = Router::new()
        .route("/", get(hello_world))
        .route("/ws", any(upgrade_to_websocket));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(PhonexError::BuildTcpListener)?;

    axum::serve(listener, app)
        .await
        .map_err(PhonexError::ServeHTTP)?;

    Ok(())
}

async fn hello_world() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRequest {
    pub candidate: String,
}

async fn upgrade_to_websocket(ws: WebSocketUpgrade) -> impl IntoResponse {
    println!("connected");
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(ws: WebSocket) {
    let mut connection = Connection::new(ws);
    connection.handle().await;
}

struct Connection {
    ws: Cell<WebSocket>,
}

impl Connection {
    fn new(ws: WebSocket) -> Self {
        Self { ws: Cell::new(ws) }
    }

    async fn handle(&mut self) {
        while let Some(message) = self.ws.get_mut().recv().await {
            if let Ok(msg) = message {
                match msg {
                    Message::Pong(v) => {
                        println!(">>> sent pong with {v:?}");
                    }
                    Message::Ping(v) => {
                        println!(">>> sent ping with {v:?}");
                    }
                    Message::Text(text) => println!("{}", text),
                    Message::Binary(text) => println!("bin: {:?}", text),
                    Message::Close(_) => break,
                }
            } else {
                break;
            };
        }
    }
}
