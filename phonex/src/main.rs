mod err;
mod message;
mod wrc;

use err::PhonexError;
use futures_util::{SinkExt, StreamExt};
use message::{CandidateRequest, SessionDescriptionRequest};
use message::{HandshakeRequest, HandshakeResponse};
use signal;
use signal::RequestType;
use std::cell::Cell;
use std::ops::ControlFlow;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tracing::instrument;
use tracing_error::ErrorLayer;
use tracing_spanned::SpanErr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

const ID: &str = "1";
const SERVER: &str = "ws://127.0.0.1:3000/ws";

#[instrument(skip_all, name = "initialize_tracing_subscriber", level = "trace")]
fn initialize_tracing_subscriber() -> Result<(), SpanErr<PhonexError>> {
    tracing_subscriber::Registry::default()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
        )
        .with(ErrorLayer::default())
        .try_init()
        .map_err(PhonexError::InitializeTracingSubscriber)?;

    Ok(())
}

#[tokio::main]
#[instrument(skip_all, name = "main", level = "trace")]
async fn main() -> Result<(), SpanErr<PhonexError>> {
    initialize_tracing_subscriber()?;

    let start_time = Instant::now();
    let (req_tx, req_rx) = mpsc::channel::<HandshakeRequest>(100);
    let (res_tx, res_rx) = mpsc::channel::<HandshakeResponse>(100);

    let mut wrc = wrc::WebRTC::new(Cell::new(req_rx), res_tx).await?;

    let webrtc_task = tokio::spawn(async move { wrc.handshake().await });
    let websocket_task = tokio::spawn(spawn_websocket(req_tx, res_rx));

    let _ = websocket_task.await;
    let _ = webrtc_task.await;

    let end_time = Instant::now();

    println!("Total time taken {:#?}", end_time - start_time);

    // wrc.close_connection().await;

    Ok(())
}

#[instrument(skip_all, name = "spawn_websocket", level = "trace")]
async fn spawn_websocket(tx: Sender<HandshakeRequest>, mut rx: Receiver<HandshakeResponse>) {
    let ws_stream = match connect_async(SERVER).await {
        Ok((stream, response)) => {
            println!("Handshake has been completed");
            println!("Server response was {response:?}");
            stream
        }
        Err(e) => {
            println!("WebSocket handshake failed with {e}!");
            return;
        }
    };

    let (mut sender, mut receiver) = ws_stream.split();

    //we can ping the server for start
    sender
        .send(Message::Ping(axum::body::Bytes::from_static(
            b"Hello, Server!",
        )))
        .await
        .expect("Can not send!");

    let mut send_task = tokio::spawn(async move {
        let m = register_message(ID.into()).unwrap();

        if sender.send(Message::Text(m.into())).await.is_err() {
            return;
        }

        loop {
            tokio::select! {
                val = rx.recv() => {
                    let response = val.unwrap();
                    match response {
                        HandshakeResponse::SessionDescriptionResponse(v) => {
                            let m = session_description_message(v.target_id, v.sdp).unwrap();

                            if let Err(e) = sender
                                .send(Message::Text(m.into()))
                                .await
                            {
                                println!("Could not send Close due to {e:?}, probably it is ok?");
                            };
                        }
                        HandshakeResponse::CandidateResponse(v) => {
                            let m = candidate_message(v.target_id, v.candidate).unwrap();

                            if let Err(e) = sender
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

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if process_message(tx.clone(), msg).await.is_break() {
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
}

#[instrument(skip_all, name = "register_message", level = "trace")]
fn register_message(id: String) -> Result<String, serde_json::Error> {
    let register_message = signal::RegisterMessage { id };
    let r = serde_json::to_string(&register_message)?;

    let message = signal::Message {
        request_type: signal::RequestType::Register,
        raw: r,
    };

    return serde_json::to_string(&message);
}

#[instrument(skip_all, name = "session_description_message", level = "trace")]
fn session_description_message(
    target_id: String,
    sdp: RTCSessionDescription,
) -> Result<String, serde_json::Error> {
    let session_description_message = signal::SessionDescriptionMessage { target_id, sdp };
    let r = serde_json::to_string(&session_description_message)?;

    let message = signal::Message {
        request_type: signal::RequestType::SessionDescription,
        raw: r,
    };

    return serde_json::to_string(&message);
}

#[instrument(skip_all, name = "candidate_message", level = "trace")]
fn candidate_message(target_id: String, candidate: String) -> Result<String, serde_json::Error> {
    let candidate_message = signal::CandidateMessage {
        target_id,
        candidate,
    };
    let r = serde_json::to_string(&candidate_message)?;

    let message = signal::Message {
        request_type: signal::RequestType::Candidate,
        raw: r,
    };

    return serde_json::to_string(&message);
}

#[instrument(skip_all, name = "process_message", level = "trace")]
async fn process_message(tx: Sender<HandshakeRequest>, msg: Message) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(message) => {
            let deserialized: signal::Message = serde_json::from_str(&message).unwrap();

            match deserialized.request_type {
                RequestType::Register => {}
                RequestType::SessionDescription => {
                    let session_description_message: signal::SessionDescriptionMessage =
                        serde_json::from_str(&deserialized.raw).unwrap();

                    println!("sdp: {:?}", session_description_message.sdp.unmarshal());

                    tx.send(HandshakeRequest::SessionDescriptionRequest(
                        SessionDescriptionRequest {
                            target_id: session_description_message.target_id,
                            sdp: session_description_message.sdp,
                        },
                    ))
                    .await
                    .unwrap();
                }
                RequestType::Candidate => {
                    let candidate_message: signal::CandidateMessage =
                        serde_json::from_str(&deserialized.raw).unwrap();

                    tx.send(HandshakeRequest::CandidateRequest(CandidateRequest {
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
