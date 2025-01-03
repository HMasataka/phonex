mod channel;
mod err;
mod r#match;
mod message;

use channel::WsChannel;
use message::RequestType;
use r#match::RegisterRequest;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{cell::Cell, net::SocketAddr};
use tokio::sync::{mpsc, Mutex};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::{any, get},
    Router,
};

use err::PhonexError;
use lazy_static::lazy_static;
use tracing_spanned::SpanErr;

use webrtc::ice_transport::ice_candidate::RTCIceCandidate;

lazy_static! {
    pub static ref PENDING_CANDIDATES: Arc<Mutex<Vec<RTCIceCandidate>>> =
        Arc::new(Mutex::new(vec![]));
}

#[tokio::main]
async fn main() -> Result<(), SpanErr<PhonexError>> {
    let (tx, rx) = mpsc::channel::<r#match::RegisterRequest>(100);
    let (tx1, rx1) = mpsc::channel::<r#match::SessionDescriptionRequest>(100);

    let mut match_server = r#match::Server::new(Cell::new(rx), Cell::new(rx1));

    tokio::spawn(async move {
        match_server.serve().await;
    });

    let channels = WsChannel {
        register_sender: Arc::new(Mutex::new(tx)),
        sdp_sender: Arc::new(Mutex::new(tx1)),
    };

    let app = Router::new()
        .route("/", get(hello_world))
        .route("/ws", any(upgrade_to_websocket))
        .with_state(Arc::new(channels));

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

async fn upgrade_to_websocket(
    State(channel): State<Arc<WsChannel>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    println!("connected");

    ws.on_upgrade(|socket| handle_socket(socket, channel))
}

async fn handle_socket(ws: WebSocket, channel: Arc<WsChannel>) {
    let mut connection = Connection::new(ws, channel);
    connection.handle().await;
}

struct Connection {
    ws: Arc<Mutex<WebSocket>>,
    chan: Arc<WsChannel>,
}

impl Connection {
    fn new(ws: WebSocket, channel: Arc<WsChannel>) -> Self {
        Self {
            ws: Arc::new(Mutex::new(ws)),
            chan: channel,
        }
    }

    async fn handle(&mut self) {
        let ws = Arc::clone(&self.ws);
        while let Some(message) = ws.lock().await.recv().await {
            if let Ok(msg) = message {
                match msg {
                    Message::Text(text) => self.handle_string(text).await,
                    Message::Binary(binary) => self.handle_binary(binary).await,
                    Message::Pong(v) => println!(">>> sent pong with {v:?}"),
                    Message::Ping(v) => println!(">>> receive ping with {v:?}"),
                    Message::Close(_) => break,
                }
            } else {
                break;
            };
        }
    }

    async fn handle_string(&mut self, message: String) {
        let deserialized: message::Message = serde_json::from_str(&message).unwrap();

        match deserialized.typ {
            RequestType::Register => {
                let tx = self.chan.register_sender.lock().await;

                tx.send(RegisterRequest {
                    id: "1".to_string(),
                    ws: Arc::clone(&self.ws),
                })
                .await
                .unwrap();
            }
            RequestType::Ping => {
                println!(">>> receive ping");
            }
            RequestType::Pong => {
                println!(">>> sent pong");
            }
        }

        println!("{:?}", deserialized);
    }

    async fn handle_binary(&mut self, message: Vec<u8>) {
        let converted: String = String::from_utf8(message.to_vec()).unwrap();
        self.handle_string(converted).await;
    }
}
