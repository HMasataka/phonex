use std::ops::ControlFlow;
use std::sync::Arc;

use crate::err::PhonexError;
use crate::message::Candidate;
use crate::message::Handshake;
use crate::message::SessionDescription;

use futures::lock::Mutex;
use futures::sink::SinkExt;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use futures_util::StreamExt;
use signal::RequestType;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::instrument;
use tracing_spanned::SpanErr;

const ID: &str = "2";

pub struct WebSocket {
    handshake_receiver: Arc<Mutex<Receiver<Handshake>>>,
    handshake_sender: Arc<Sender<Handshake>>,
    ws_sender: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    ws_receiver: Arc<Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
}

impl WebSocket {
    #[instrument(skip_all, name = "webrtc_new", level = "trace")]
    pub fn new(
        handshake_receiver: Receiver<Handshake>,
        handshake_sender: Sender<Handshake>,
        ws_sender: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        ws_receiver: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) -> Result<Self, SpanErr<PhonexError>> {
        Ok(Self {
            handshake_receiver: Arc::new(Mutex::new(handshake_receiver)),
            handshake_sender: Arc::new(handshake_sender),
            ws_sender: Arc::new(Mutex::new(ws_sender)),
            ws_receiver: Arc::new(Mutex::new(ws_receiver)),
        })
    }

    #[instrument(skip_all, name = "websocket_spawn", level = "trace")]
    pub async fn spawn(&mut self) -> Result<(), SpanErr<PhonexError>> {
        self.ping().await?;
        self.register().await?;

        let handshake_receiver = Arc::clone(&self.handshake_receiver);
        let ws_sender = Arc::clone(&self.ws_sender);

        let mut send_task = tokio::spawn(async move {
            let mut handshake_receiver = handshake_receiver.lock().await;

            loop {
                tokio::select! {
                    val = handshake_receiver.recv() => {
                        let response = val.unwrap();
                        let ws_sender = Arc::clone(&ws_sender);

                        if send_message(response, ws_sender).await.is_break(){
                            break;
                        }

                    }
                }
            }
        });

        let ws_receiver = Arc::clone(&self.ws_receiver);
        let handshake_sender = Arc::clone(&self.handshake_sender);

        let mut recv_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_receiver.lock().await.next().await {
                if process_message(Arc::clone(&handshake_sender), msg)
                    .await
                    .is_break()
                {
                    break;
                }
            }
        });

        tokio::select! {
            _ = (&mut send_task) => {
                recv_task.abort();
            },
            _ = (&mut recv_task) => {
                send_task.abort();
            }
        }

        Ok(())
    }

    #[instrument(skip_all, name = "websocket_ping", level = "trace")]
    async fn ping(&mut self) -> Result<(), SpanErr<PhonexError>> {
        let mut ws_sender = self.ws_sender.lock().await;

        ws_sender
            .send(Message::Ping(axum::body::Bytes::from_static(
                b"Hello, Server!",
            )))
            .await
            .expect("Can not send!");

        Ok(())
    }

    #[instrument(skip_all, name = "websocket_register", level = "trace")]
    async fn register(&mut self) -> Result<(), SpanErr<PhonexError>> {
        let register = signal::Message::new_register_message(ID.into())
            .map_err(PhonexError::WrapCommonError)?
            .try_to_string()
            .map_err(PhonexError::WrapCommonError)?;

        let mut ws_sender = self.ws_sender.lock().await;

        ws_sender
            .send(Message::Text(register.into()))
            .await
            .map_err(PhonexError::SendWebSocketMessage)?;

        Ok(())
    }
}

#[instrument(skip_all, name = "send_message", level = "trace")]
async fn send_message(
    handshake: Handshake,
    ws_sender: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
) -> ControlFlow<(), ()> {
    match handshake {
        Handshake::SessionDescription(v) => {
            let m = match signal::Message::new_session_description_message(v.target_id, v.sdp) {
                Ok(m) => {
                    let s = match m.try_to_string() {
                        Ok(s) => s,
                        Err(e) => {
                            println!("convert error: {e}");
                            return ControlFlow::Break(());
                        }
                    };
                    s
                }
                Err(e) => {
                    println!("build sdp error: {e}");
                    return ControlFlow::Break(());
                }
            };

            if let Err(e) = ws_sender.lock().await.send(Message::Text(m.into())).await {
                println!("Could not send Close due to {e:?}, probably it is ok?");
                return ControlFlow::Break(());
            };
        }
        Handshake::Candidate(v) => {
            let m = match signal::Message::new_candidate_message(v.target_id, v.candidate) {
                Ok(m) => {
                    let s = match m.try_to_string() {
                        Ok(s) => s,
                        Err(e) => {
                            println!("convert error: {e}");
                            return ControlFlow::Break(());
                        }
                    };

                    s
                }
                Err(e) => {
                    println!("build candidate error: {e}");
                    return ControlFlow::Break(());
                }
            };

            if let Err(e) = ws_sender.lock().await.send(Message::Text(m.into())).await {
                println!("Could not send Close due to {e:?}, probably it is ok?");
                return ControlFlow::Break(());
            };
        }
    }

    ControlFlow::Continue(())
}

#[instrument(skip_all, name = "process_message", level = "trace")]
async fn process_message(
    handshake_sender: Arc<Sender<Handshake>>,
    msg: Message,
) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(message) => {
            let deserialized: signal::Message = serde_json::from_str(&message).unwrap();

            match deserialized.request_type {
                RequestType::Register => {}
                RequestType::SessionDescription => {
                    let session_description_message: signal::SessionDescriptionMessage =
                        serde_json::from_str(&deserialized.raw).unwrap();

                    println!("sdp: {:?}", session_description_message.sdp.unmarshal());

                    handshake_sender
                        .send(Handshake::SessionDescription(SessionDescription {
                            target_id: session_description_message.target_id,
                            sdp: session_description_message.sdp,
                        }))
                        .await
                        .unwrap();
                }
                RequestType::Candidate => {
                    let candidate_message: signal::CandidateMessage =
                        serde_json::from_str(&deserialized.raw).unwrap();

                    handshake_sender
                        .send(Handshake::Candidate(Candidate {
                            target_id: candidate_message.target_id,
                            candidate: candidate_message.candidate,
                        }))
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
        Message::Binary(d) => {
            println!(">>> got {} bytes: {:?}", d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> got close with code {} and reason `{}`",
                    cf.code, cf.reason
                );
            } else {
                println!(">>> got close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }

        Message::Pong(v) => {
            println!(">>> got pong with {v:?}");
        }
        Message::Ping(v) => {
            println!(">>> got ping with {v:?}");
        }

        Message::Frame(_) => {
            unreachable!("This is never supposed to happen")
        }
    }

    ControlFlow::Continue(())
}
