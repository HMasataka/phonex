pub mod err;
pub mod r#match;
pub mod match_server;
pub mod message;

use futures::stream::{SplitSink, StreamExt};
use futures::SinkExt;
use match_server::Server;
use message::{CandidateMessage, RequestType, SessionDescriptionMessage};
use r#match::{MatchRegisterRequest, MatchRequest, MatchResponse};
use std::sync::Arc;
use std::{cell::Cell, net::SocketAddr};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};
use tracing::instrument;
use tracing_error::ErrorLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::{any, get},
    Router,
};

use err::SignalError;
use lazy_static::lazy_static;
use tracing_spanned::SpanErr;

use webrtc::ice_transport::ice_candidate::RTCIceCandidate;

lazy_static! {
    pub static ref PENDING_CANDIDATES: Arc<Mutex<Vec<RTCIceCandidate>>> =
        Arc::new(Mutex::new(vec![]));
}

#[instrument(skip_all, name = "initialize_tracing_subscriber", level = "trace")]
fn initialize_tracing_subscriber() -> Result<(), SpanErr<SignalError>> {
    tracing_subscriber::Registry::default()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
        )
        .with(ErrorLayer::default())
        .try_init()
        .map_err(SignalError::InitializeTracingSubscriber)?;

    Ok(())
}

#[tokio::main]
#[instrument(skip_all, name = "main", level = "trace")]
async fn main() -> Result<(), SpanErr<SignalError>> {
    initialize_tracing_subscriber()?;

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
        .map_err(SignalError::BuildTcpListener)?;

    axum::serve(listener, app)
        .await
        .map_err(SignalError::ServeHTTP)?;

    Ok(())
}

#[instrument(skip_all, name = "hello_world", level = "trace")]
async fn hello_world() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

#[instrument(skip_all, name = "upgrade_to_websocket", level = "trace")]
async fn upgrade_to_websocket(
    State(channel): State<Sender<MatchRequest>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    println!("connected");

    ws.on_upgrade(move |socket| handle_socket(socket, channel))
}

#[instrument(skip_all, name = "handle_socket", level = "trace")]
async fn handle_socket(ws: WebSocket, match_sender: Sender<MatchRequest>) {
    let (sender, mut receiver) = ws.split();
    let (tx, rx) = mpsc::channel::<MatchResponse>(100);
    let request_handler = RequestHandler::new(match_sender, tx);
    let mut response_handler = ResponseHandler::new(sender, Cell::new(rx));

    tokio::spawn(async move {
        let result = response_handler.serve().await;
        if result.is_err() {
            //TODO error handling
        }
    });

    while let Some(message) = receiver.next().await {
        if let Ok(msg) = message {
            match msg {
                Message::Text(text) => {
                    let result = request_handler.clone().string(text).await;
                    if result.is_err() {
                        // TODO error handling
                        break;
                    }
                }
                Message::Binary(binary) => {
                    let result = request_handler.clone().binary(binary).await;
                    if result.is_err() {
                        // TODO error handling
                        break;
                    }
                }
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
    #[instrument(skip_all, name = "request_handler_new", level = "trace")]
    fn new(req: Sender<MatchRequest>, res: Sender<MatchResponse>) -> Self {
        Self {
            request_sender: req,
            response_sender: res,
        }
    }

    #[instrument(skip_all, name = "request_handler_string", level = "trace")]
    async fn string(self, message: String) -> Result<(), SignalError> {
        let deserialized: message::Message =
            serde_json::from_str(&message).map_err(SignalError::SerializeToJSONError)?;

        match deserialized.request_type {
            RequestType::Register => {
                let register_message: message::RegisterMessage =
                    serde_json::from_str(&deserialized.raw)
                        .map_err(SignalError::DeserializeJSONError)?;

                self.request_sender
                    .send(MatchRequest::Register(MatchRegisterRequest {
                        id: register_message.id,
                        chan: self.response_sender,
                    }))
                    .await
                    .map_err(SignalError::SendMatchRequest)?;
            }
            RequestType::SessionDescription => {
                let session_description_message: message::SessionDescriptionMessage =
                    serde_json::from_str(&deserialized.raw)
                        .map_err(SignalError::DeserializeJSONError)?;

                self.request_sender
                    .send(MatchRequest::SessionDescription(
                        session_description_message.into(),
                    ))
                    .await
                    .map_err(SignalError::SendMatchRequest)?;
            }
            RequestType::Candidate => {
                let candidate_message: message::CandidateMessage =
                    serde_json::from_str(&deserialized.raw)
                        .map_err(SignalError::DeserializeJSONError)?;

                self.request_sender
                    .send(MatchRequest::Candidate(candidate_message.into()))
                    .await
                    .map_err(SignalError::SendMatchRequest)?;
            }
            RequestType::Ping => {
                println!(">>> receive ping");
            }
            RequestType::Pong => {
                println!(">>> sent pong");
            }
        }

        println!("{:?}", deserialized);

        Ok(())
    }

    #[instrument(skip_all, name = "request_handler_binary", level = "trace")]
    async fn binary(self, message: Vec<u8>) -> Result<(), SignalError> {
        let converted: String =
            String::from_utf8(message.to_vec()).map_err(SignalError::ConvertToStringError)?;
        return self.string(converted).await;
    }
}

pub struct ResponseHandler {
    ws: SplitSink<WebSocket, Message>,
    response_receiver: Cell<Receiver<MatchResponse>>,
}

impl ResponseHandler {
    #[instrument(skip_all, name = "response_handler_new", level = "trace")]
    fn new(
        ws: SplitSink<WebSocket, Message>,
        response_receiver: Cell<Receiver<MatchResponse>>,
    ) -> Self {
        Self {
            ws,
            response_receiver,
        }
    }

    #[instrument(skip_all, name = "response_handler_serve", level = "trace")]
    async fn serve(&mut self) -> Result<(), SignalError> {
        loop {
            tokio::select! {
                val = self.response_receiver.get_mut().recv() => {
                    let value = val.unwrap();

                    match value{
                        MatchResponse::SessionDescription(value) => {
                            println!("{:?}", value);

                            let raw = SessionDescriptionMessage{
                                target_id: value.target_id,
                                sdp: value.sdp,
                            };

                            let response = message::Message{
                                request_type: RequestType::SessionDescription,
                                raw: serde_json::to_string(&raw).map_err(SignalError::SerializeToJSONError)?,
                            };

                            let text = serde_json::to_string(&response).map_err(SignalError::SerializeToJSONError)?;

                            self.ws.send(Message::Text(text.into())).await.map_err(SignalError::WriteWebsocket)?;
                        }
                        MatchResponse::Candidate(value) => {
                            println!("{:?}", value);

                            let raw = CandidateMessage{
                                target_id: value.target_id,
                                candidate: value.candidate,
                            };

                            let response = message::Message{
                                request_type: RequestType::Candidate,
                                raw: serde_json::to_string(&raw).map_err(SignalError::SerializeToJSONError)?,
                            };

                            let text = serde_json::to_string(&response).map_err(SignalError::SerializeToJSONError)?;

                            self.ws.send(Message::Text(text.into())).await.map_err(SignalError::WriteWebsocket)?;
                        }
                    }
                }
            }
        }
    }
}
