mod err;
mod handshake;

use std::sync::Arc;
use std::sync::Weak;

use anyhow::Result;
use clap::Parser;
use dotenvy::dotenv;
use err::PhonexError;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tracing::instrument;
use tracing_error::ErrorLayer;
use tracing_spanned::SpanErr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::OnMessageHdlrFn;
use webrtc::data_channel::OnOpenHdlrFn;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_gatherer::OnLocalCandidateHdlrFn;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::OnPeerConnectionStateChangeHdlrFn;
use webrtc::peer_connection::{math_rand_alpha, RTCPeerConnection};

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref PEER_CONNECTION_MUTEX: Arc<Mutex<Option<Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(None));
    static ref PENDING_CANDIDATES: Arc<Mutex<Vec<RTCIceCandidate>>> = Arc::new(Mutex::new(vec![]));
    static ref ADDRESS: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[clap(
        short = 'o',
        long = "offer_address",
        env,
        default_value = "0.0.0.0:50000"
    )]
    offer_address: String,
    #[clap(
        short = 'a',
        long = "answer_address",
        env,
        default_value = "0.0.0.0:60000"
    )]
    answer_address: String,
}

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

#[instrument(skip_all, name = "initialize_peer_connection", level = "trace")]
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

#[instrument(skip_all, name = "on_ice_candidate", level = "trace")]
fn on_ice_candidate(
    peer_connection: Weak<RTCPeerConnection>,
    pending_candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
    answer_address: String,
) -> Result<OnLocalCandidateHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |c: Option<RTCIceCandidate>| {
        println!("on ice candidate: {:?}", c);

        let pc = peer_connection.clone();
        let pending_candidates = Arc::clone(&pending_candidates);
        let addr = answer_address.clone();
        Box::pin(async move {
            if let Some(c) = c {
                if let Some(pc) = pc.upgrade() {
                    let desc = pc.remote_description().await;
                    if desc.is_none() {
                        let mut cs = pending_candidates.lock().await;
                        cs.push(c);
                    } else if let Err(err) = handshake::signal_candidate(&addr, &c).await {
                        panic!("{}", err);
                    }
                }
            }
        })
    }))
}

#[instrument(skip_all, name = "on_peer_connection_state_change", level = "trace")]
fn on_peer_connection_state_change(
    done_tx: Sender<()>,
) -> Result<OnPeerConnectionStateChangeHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |s: RTCPeerConnectionState| {
        println!("Peer Connection State has changed: {s}");

        if s == RTCPeerConnectionState::Failed {
            println!("Peer Connection has gone to failed exiting");
            let _ = done_tx.try_send(());
        }

        Box::pin(async {})
    }))
}

#[instrument(skip_all, name = "on_open", level = "trace")]
fn on_open(data_channel: Arc<RTCDataChannel>) -> Result<OnOpenHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move || {
        println!("Data channel '{}'-'{}' open. Random messages will now be sent to any connected DataChannels every 5 seconds", data_channel.label(), data_channel.id());

        let dc = Arc::clone(&data_channel);
        Box::pin(async move {
            let mut result = Result::<usize, PhonexError>::Ok(0);

            while result.is_ok() {
                let timeout = tokio::time::sleep(Duration::from_secs(5));
                tokio::pin!(timeout);

                tokio::select! {
                    _ = timeout.as_mut() =>{
                        let message = math_rand_alpha(15);
                        println!("Sending '{message}'");
                        result = dc.send_text(message).await.map_err(PhonexError::SendMessage);
                    }
                };
            }
        })
    }))
}

#[instrument(skip_all, name = "on_message", level = "trace")]
fn on_message(data_channel_label: String) -> Result<OnMessageHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |msg: DataChannelMessage| {
        let msg_str = String::from_utf8(msg.data.to_vec())
            .map_err(PhonexError::ConvertByteToString)
            .unwrap();
        println!("Message from DataChannel '{data_channel_label}': '{msg_str}'");
        Box::pin(async {})
    }))
}

#[instrument(skip_all, name = "on_peer_connection_state_change", level = "trace")]
async fn sdp_offer(addr: String, offer: RTCSessionDescription) -> Result<(), SpanErr<PhonexError>> {
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/sdp"))
        .json(&offer)
        .send()
        .await
        .map_err(PhonexError::SendHTTPRequest)?;

    println!("Response: {}", resp.status());

    Ok(())
}

#[tokio::main]
#[instrument(skip_all, name = "main", level = "trace")]
async fn main() -> Result<(), SpanErr<PhonexError>> {
    initialize_tracing_subscriber()?;

    let _ = dotenv();

    let args = Args::parse();

    println!("{:?}", args);

    {
        let mut oa = ADDRESS.lock().await;
        oa.clone_from(&args.answer_address);
    }

    let peer_connection = initialize_peer_connection().await?;

    peer_connection.on_ice_candidate(on_ice_candidate(
        Arc::downgrade(&peer_connection),
        Arc::clone(&handshake::PENDING_CANDIDATES),
        args.answer_address.clone(),
    )?);

    {
        let mut pcm = PEER_CONNECTION_MUTEX.lock().await;
        *pcm = Some(Arc::clone(&peer_connection));
    }

    let data_channel = peer_connection
        .create_data_channel("data", None)
        .await
        .map_err(PhonexError::CreateNewDataChannel)?;

    let pc = Arc::clone(&peer_connection);
    tokio::spawn(async move {
        handshake::serve(args.offer_address, &pc).await.unwrap();
    });

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    peer_connection.on_peer_connection_state_change(on_peer_connection_state_change(done_tx)?);

    data_channel.on_open(on_open(Arc::clone(&data_channel))?);
    data_channel.on_message(on_message(data_channel.label().to_owned())?);

    let offer = peer_connection
        .create_offer(None)
        .await
        .map_err(PhonexError::CreateNewOffer)?;

    peer_connection
        .set_local_description(offer.clone())
        .await
        .map_err(PhonexError::SetLocalDescription)?;

    sdp_offer(args.answer_address, offer).await?;

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };

    peer_connection.close().await.unwrap();

    Ok(())
}
