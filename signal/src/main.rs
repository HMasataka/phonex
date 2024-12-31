mod err;
mod r#match;
mod message;

use futures::stream::StreamExt;
use message::RequestType;
use r#match::{MatchRequest, MatchResponse, Server};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{cell::Cell, net::SocketAddr};
use tokio::sync::mpsc::Sender;
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
    let (tx, rx) = mpsc::channel::<MatchRequest>(100);

    let mut match_server = Server::new(Cell::new(rx));

    tokio::spawn(async move {
        match_server.serve().await;
    });

    let app = Router::new()
        .route("/", get(hello_world))
        .route("/ws", any(upgrade_to_websocket))
        .with_state(tx);

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
    State(channel): State<Sender<MatchRequest>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    println!("connected");

    ws.on_upgrade(move |socket| handle_socket(socket, channel))
}

async fn handle_socket(ws: WebSocket, sender: Sender<MatchRequest>) {
    let (mut _sender, mut receiver) = ws.split();
    let (tx, _rx) = mpsc::channel::<MatchResponse>(100);
    let handler = Handler::new(sender, tx);

    while let Some(message) = receiver.next().await {
        if let Ok(msg) = message {
            match msg {
                Message::Text(text) => handler.clone().string(text).await,
                Message::Binary(binary) => handler.clone().binary(binary).await,
                Message::Pong(v) => println!(">>> sent pong with {v:?}"),
                Message::Ping(v) => println!(">>> receive ping with {v:?}"),
                Message::Close(_) => break,
            }
        } else {
            break;
        };
    }
}

#[derive(Clone)]
pub struct Handler {
    request_sender: Sender<MatchRequest>,
    response_sender: Sender<MatchResponse>,
}

impl Handler {
    fn new(req: Sender<MatchRequest>, res: Sender<MatchResponse>) -> Self {
        Self {
            request_sender: req,
            response_sender: res,
        }
    }

    async fn string(self, message: String) {
        let deserialized: message::Message = serde_json::from_str(&message).unwrap();

        match deserialized.typ {
            RequestType::Register => {
                self.request_sender
                    .send(MatchRequest {
                        id: "1".to_string(),
                        chan: self.response_sender,
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

    async fn binary(self, message: Vec<u8>) {
        let converted: String = String::from_utf8(message.to_vec()).unwrap();
        self.string(converted).await;
    }
}
