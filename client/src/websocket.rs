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
    rx: Arc<Mutex<Receiver<Handshake>>>,
    tx: Arc<Sender<Handshake>>,
    sender: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    receiver: Arc<Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
}

impl WebSocket {
    #[instrument(skip_all, name = "webrtc_new", level = "trace")]
    pub fn new(
        rx: Receiver<Handshake>,
        tx: Sender<Handshake>,
        sender: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        receiver: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) -> Result<Self, SpanErr<PhonexError>> {
        Ok(Self {
            rx: Arc::new(Mutex::new(rx)),
            tx: Arc::new(tx),
            sender: Arc::new(Mutex::new(sender)),
            receiver: Arc::new(Mutex::new(receiver)),
        })
    }

    #[instrument(skip_all, name = "websocket_spawn", level = "trace")]
    pub async fn spawn(&mut self) -> Result<(), SpanErr<PhonexError>> {
        self.ping().await?;
        self.register().await?;

        let sender = Arc::clone(&self.sender);
        let rx = Arc::clone(&self.rx);

        let mut send_task = tokio::spawn(async move {
            let mut rx = rx.lock().await;

            loop {
                tokio::select! {
                    val = rx.recv() => {
                        let response = val.unwrap();
                        match response {
                            Handshake::SessionDescription(v) => {
                                let m = signal::Message::new_session_description_message(v.target_id, v.sdp).unwrap().try_to_string().unwrap();

                                if let Err(e) = sender.lock().await
                                    .send(Message::Text(m.into()))
                                    .await
                                {
                                    println!("Could not send Close due to {e:?}, probably it is ok?");
                                };
                            }
                            Handshake::Candidate(v) => {
                                let m = signal::Message::new_candidate_message(v.target_id, v.candidate).unwrap().try_to_string().unwrap();

                                if let Err(e) = sender.lock().await
                                    .send(Message::Text(m.into()))
                                    .await
                                {
                                    println!("Could not send Close due to {e:?}, probably it is ok?");
                                };
                            }
                        }
                    }
                }
            }
        });

        let receiver = Arc::clone(&self.receiver);
        let tx = Arc::clone(&self.tx);

        let mut recv_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.lock().await.next().await {
                if process_message(Arc::clone(&tx), msg).await.is_break() {
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
        let mut sender = self.sender.lock().await;

        sender
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
            .unwrap()
            .try_to_string()
            .unwrap();

        let mut sender = self.sender.lock().await;

        sender
            .send(Message::Text(register.into()))
            .await
            .map_err(PhonexError::SendWebSocketMessage)?;

        Ok(())
    }
}

#[instrument(skip_all, name = "process_message", level = "trace")]
async fn process_message(tx: Arc<Sender<Handshake>>, msg: Message) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(message) => {
            let deserialized: signal::Message = serde_json::from_str(&message).unwrap();

            match deserialized.request_type {
                RequestType::Register => {}
                RequestType::SessionDescription => {
                    let session_description_message: signal::SessionDescriptionMessage =
                        serde_json::from_str(&deserialized.raw).unwrap();

                    println!("sdp: {:?}", session_description_message.sdp.unmarshal());

                    tx.send(Handshake::SessionDescription(SessionDescription {
                        target_id: session_description_message.target_id,
                        sdp: session_description_message.sdp,
                    }))
                    .await
                    .unwrap();
                }
                RequestType::Candidate => {
                    let candidate_message: signal::CandidateMessage =
                        serde_json::from_str(&deserialized.raw).unwrap();

                    tx.send(Handshake::Candidate(Candidate {
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
