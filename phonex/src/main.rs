mod err;

use err::PhonexError;
use futures_util::{SinkExt, StreamExt};
use signal;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Instant;
use tokio_tungstenite::tungstenite::Utf8Bytes;
use tracing_spanned::SpanErr;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_gatherer::OnLocalCandidateHdlrFn;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message},
};

const N_CLIENTS: usize = 2; //set to desired number
const SERVER: &str = "ws://127.0.0.1:3000/ws";

async fn initialize_peer_connection() -> Result<Arc<RTCPeerConnection>, SpanErr<PhonexError>> {
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let mut m = MediaEngine::default();
    m.register_default_codecs()
        .map_err(PhonexError::InitializeRegistry)?;

    let mut registry = Registry::new();
    registry =
        register_default_interceptors(registry, &mut m).map_err(PhonexError::InitializeRegistry)?;

    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    let peer_connection = Arc::new(
        api.new_peer_connection(config)
            .await
            .map_err(PhonexError::CreateNewPeerConnection)?,
    );

    Ok(peer_connection)
}

fn on_ice_candidate() -> Result<OnLocalCandidateHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |c: Option<RTCIceCandidate>| {
        println!("on ice candidate: {:?}", c);

        Box::pin(async move {
            if let Some(candidate) = c {
                // TODO send candidate
            }
        })
    }))
}

#[tokio::main]
async fn main() -> Result<(), SpanErr<PhonexError>> {
    let start_time = Instant::now();

    let websocket_task = tokio::spawn(spawn_client());

    let peer_connection = initialize_peer_connection().await?;

    peer_connection.on_ice_candidate(on_ice_candidate()?);

    let offer = peer_connection
        .create_offer(None)
        .await
        .map_err(PhonexError::CreateNewOffer)?;

    peer_connection
        .set_local_description(offer.clone())
        .await
        .map_err(PhonexError::SetLocalDescription)?;

    // TODO send offer

    let _ = websocket_task.await;

    let end_time = Instant::now();

    println!("Total time taken {:#?}", end_time - start_time);

    peer_connection.close().await.unwrap();

    Ok(())
}

//creates a client. quietly exits on failure.
async fn spawn_client() {
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
        let m = register_message("1".into()).unwrap();

        if sender.send(Message::Text(m.into())).await.is_err() {
            return;
        }

        let m = register_message("1".into()).unwrap();

        if sender.send(Message::Text(m.into())).await.is_err() {
            return;
        }

        let m = candidate_message("1".into(), "candidate".into()).unwrap();

        if sender.send(Message::Text(m.into())).await.is_err() {
            return;
        }

        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        // When we are done we may want our client to close connection cleanly.
        if let Err(e) = sender
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: Utf8Bytes::from_static("Goodbye"),
            })))
            .await
        {
            println!("Could not send Close due to {e:?}, probably it is ok?");
        };
    });

    //receiver just prints whatever it gets
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            // print message and break if instructed to do so
            if process_message(msg).is_break() {
                break;
            }
        }
    });

    //wait for either task to finish and kill the other task
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        },
        _ = (&mut recv_task) => {
            send_task.abort();
        }
    }
}

fn register_message(id: String) -> Result<String, serde_json::Error> {
    let register_message = signal::RegisterMessage { id };
    let r = serde_json::to_string(&register_message)?;

    let message = signal::Message {
        request_type: signal::RequestType::Register,
        raw: r,
    };

    return serde_json::to_string(&message);
}

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

fn process_message(msg: Message) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            println!(">>> got str: {t:?}");
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
        // Just as with axum server, the underlying tungstenite websocket library
        // will handle Ping for you automagically by replying with Pong and copying the
        // v according to spec. But if you need the contents of the pings you can see them here.
        Message::Ping(v) => {
            println!(">>> got ping with {v:?}");
        }

        Message::Frame(_) => {
            unreachable!("This is never supposed to happen")
        }
    }
    ControlFlow::Continue(())
}
