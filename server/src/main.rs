mod data_channel;
mod err;
mod message;
mod websocket;
mod wrc;

use err::PhonexError;
use futures_util::StreamExt;
use message::Handshake;
use std::cell::Cell;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::instrument;
use tracing_error::ErrorLayer;
use tracing_spanned::SpanErr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

use tokio_tungstenite::connect_async;

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
    let (req_tx, req_rx) = mpsc::channel::<Handshake>(100);
    let (res_tx, res_rx) = mpsc::channel::<Handshake>(100);

    let (ws_stream, _) = connect_async(SERVER)
        .await
        .map_err(PhonexError::ConnectWebsocket)?;

    let (sender, receiver) = ws_stream.split();

    let mut w = websocket::WebSocket::new(res_rx, req_tx, sender, receiver)?;
    let mut wrc = wrc::WebRTC::new(Cell::new(req_rx), res_tx).await?;

    let webrtc_task = tokio::spawn(async move { wrc.handshake().await });
    let websocket_task = tokio::spawn(async move { w.spawn().await });

    let _ = websocket_task.await;
    let _ = webrtc_task.await;

    let end_time = Instant::now();

    println!("Total time taken {:#?}", end_time - start_time);

    // wrc.close_connection().await;

    Ok(())
}
