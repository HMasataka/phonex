mod err;
mod handshake;

use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;

use anyhow::Result;
use clap::Parser;
use dotenvy::dotenv;
use err::PhonexError;
use tokio::time::Duration;
use tracing::instrument;
use tracing_error::ErrorLayer;
use tracing_spanned::SpanErr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::RTCIceCandidate;
use webrtc::ice_transport::ice_gatherer::OnLocalCandidateHdlrFn;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::math_rand_alpha;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;

#[macro_use]
extern crate lazy_static;

lazy_static! {}

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
async fn on_ice_candidate(
    peer_connection: Weak<RTCPeerConnection>,
    pending_candidates: Arc<Mutex<Vec<RTCIceCandidate>>>,
    offer_address: String,
) -> Result<OnLocalCandidateHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |c: Option<RTCIceCandidate>| {
        println!("on ice candidate: {:?}", c);

        let pc = peer_connection.clone();
        let pending_candidates = Arc::clone(&pending_candidates);
        let addr = offer_address.clone();
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

#[tokio::main]
#[instrument(skip_all, name = "main", level = "trace")]
async fn main() -> Result<(), SpanErr<PhonexError>> {
    initialize_tracing_subscriber()?;

    let _ = dotenv();
    let args = Args::parse();
    println!("{:?}", args);

    let peer_connection = initialize_peer_connection().await?;

    let pc = Arc::downgrade(&peer_connection);
    let pending_candidates = Arc::clone(&handshake::PENDING_CANDIDATES);

    let offer_address = args.offer_address.clone();

    peer_connection
        .on_ice_candidate(on_ice_candidate(pc, pending_candidates, offer_address).await?);

    let pc = Arc::clone(&peer_connection);
    tokio::spawn(async move {
        handshake::serve(args.offer_address, args.answer_address, &pc).await;
    });

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        println!("Peer Connection State has changed: {s}");

        if s == RTCPeerConnectionState::Failed {
            println!("Peer Connection has gone to failed exiting");
            let _ = done_tx.try_send(());
        }

        Box::pin(async {})
    }));

    peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
        let d_label = d.label().to_owned();
        let d_id = d.id();
        println!("New DataChannel {d_label} {d_id}");

        Box::pin(async move{
            let d2 =  Arc::clone(&d);
            let d_label2 = d_label.clone();
            let d_id2 = d_id;
            d.on_open(Box::new(move || {
                println!("Data channel '{d_label2}'-'{d_id2}' open. Random messages will now be sent to any connected DataChannels every 5 seconds");
                Box::pin(async move {
                    let mut result = Result::<usize>::Ok(0);
                    while result.is_ok() {
                        let timeout = tokio::time::sleep(Duration::from_secs(5));
                        tokio::pin!(timeout);

                        tokio::select! {
                            _ = timeout.as_mut() =>{
                                let message = math_rand_alpha(15);
                                println!("Sending '{message}'");
                                result = d2.send_text(message).await.map_err(Into::into);
                            }
                        };
                    }
                })
            }));

            d.on_message(Box::new(move |msg: DataChannelMessage| {
               let msg_str = String::from_utf8(msg.data.to_vec()).unwrap();
               println!("Message from DataChannel '{d_label}': '{msg_str}'");
               Box::pin(async{})
           }));
        })
    }));

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
