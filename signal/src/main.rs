mod err;
mod r#match;
mod match_server;

use futures::stream::{SplitSink, StreamExt};
use futures::SinkExt;
use match_server::Server;
use r#match::{MatchRegisterRequest, MatchRequest, MatchResponse};
use signal::{CandidateMessage, RequestType, SessionDescriptionMessage};
use std::sync::Arc;
use std::{cell::Cell, net::SocketAddr};
use tokio::sync::mpsc::{Receiver, Sender};
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

async fn upgrade_to_websocket(
    State(channel): State<Sender<MatchRequest>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    println!("connected");

    ws.on_upgrade(move |socket| handle_socket(socket, channel))
}

async fn handle_socket(ws: WebSocket, match_sender: Sender<MatchRequest>) {
    let (sender, mut receiver) = ws.split();
    let (tx, rx) = mpsc::channel::<MatchResponse>(100);
    let request_handler = RequestHandler::new(match_sender, tx);
    let mut response_handler = ResponseHandler::new(sender, Cell::new(rx));

    tokio::spawn(async move {
        response_handler.serve().await;
    });

    while let Some(message) = receiver.next().await {
        if let Ok(msg) = message {
            match msg {
                Message::Text(text) => request_handler.clone().string(text).await,
                Message::Binary(binary) => request_handler.clone().binary(binary).await,
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
pub struct RequestHandler {
    request_sender: Sender<MatchRequest>,
    response_sender: Sender<MatchResponse>,
}

impl RequestHandler {
    fn new(req: Sender<MatchRequest>, res: Sender<MatchResponse>) -> Self {
        Self {
            request_sender: req,
            response_sender: res,
        }
    }

    async fn string(self, message: String) {
        let deserialized: signal::Message = serde_json::from_str(&message).unwrap();

        match deserialized.request_type {
            RequestType::Register => {
                let register_message: signal::RegisterMessage =
                    serde_json::from_str(&deserialized.raw).unwrap();

                self.request_sender
                    .send(MatchRequest::Register(MatchRegisterRequest {
                        id: register_message.id,
                        chan: self.response_sender,
                    }))
                    .await
                    .unwrap();
            }
            RequestType::SessionDescription => {
                let session_description_message: signal::SessionDescriptionMessage =
                    serde_json::from_str(&deserialized.raw).unwrap();

                self.request_sender
                    .send(MatchRequest::SessionDescription(
                        session_description_message.into(),
                    ))
                    .await
                    .unwrap();
            }
            RequestType::Candidate => {
                let candidate_message: signal::CandidateMessage =
                    serde_json::from_str(&deserialized.raw).unwrap();

                self.request_sender
                    .send(MatchRequest::Candidate(candidate_message.into()))
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

pub struct ResponseHandler {
    ws: SplitSink<WebSocket, Message>,
    response_receiver: Cell<Receiver<MatchResponse>>,
}

impl ResponseHandler {
    fn new(
        ws: SplitSink<WebSocket, Message>,
        response_receiver: Cell<Receiver<MatchResponse>>,
    ) -> Self {
        Self {
            ws,
            response_receiver,
        }
    }

    async fn serve(&mut self) {
        loop {
            tokio::select! {
                val = self.response_receiver.get_mut().recv() => {
                    let value = val.unwrap();

                    match value{
                        MatchResponse::SessionDescription(value) => {
                            println!("{:?}", value);

                            let raw = SessionDescriptionMessage{
                                target_id: "".into(),
                                sdp: value.sdp,
                            };

                            let response =signal::Message{
                                request_type: RequestType::SessionDescription,
                                raw: serde_json::to_string(&raw).unwrap(),
                            };

                            let text = serde_json::to_string(&response).unwrap();

                            self.ws.send(Message::Text(text.into())).await.unwrap();
                        }
                        MatchResponse::Candidate(value) => {
                            println!("{:?}", value);

                            let raw = CandidateMessage{
                                target_id: "".into(),
                                candidate: value.candidate,
                            };

                            let response =signal::Message{
                                request_type: RequestType::SessionDescription,
                                raw: serde_json::to_string(&raw).unwrap(),
                            };

                            let text = serde_json::to_string(&response).unwrap();

                            self.ws.send(Message::Text(text.into())).await.unwrap();
                        }
                    }
                }
            }
        }
    }
}
