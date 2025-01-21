use std::sync::Arc;

use tokio::time::Duration;
use tracing::instrument;
use tracing_spanned::SpanErr;
use webrtc::{
    data_channel::{
        data_channel_message::DataChannelMessage, OnMessageHdlrFn, OnOpenHdlrFn, RTCDataChannel,
    },
    peer_connection::{math_rand_alpha, RTCPeerConnection},
};

use crate::err::PhonexError;

#[instrument(skip_all, name = "initialize_channel", level = "trace")]
pub async fn initialize_data_channel(
    peer_connection: Arc<RTCPeerConnection>,
    label: &str,
) -> Result<(), SpanErr<PhonexError>> {
    let data_channel = peer_connection
        .create_data_channel(label, None)
        .await
        .map_err(PhonexError::CreateNewDataChannel)?;

    data_channel.on_open(data_channel_on_open(Arc::clone(&data_channel))?);
    data_channel.on_message(data_channel_on_message(label.to_string())?);

    Ok(())
}

#[instrument(skip_all, name = "data_channel_on_open", level = "trace")]
fn data_channel_on_open(
    data_channel: Arc<RTCDataChannel>,
) -> Result<OnOpenHdlrFn, SpanErr<PhonexError>> {
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
                        result = dc.send_text(message).await.map_err(PhonexError::SendWebRTCMessage);
                    }
                };
            }
        })
    }))
}

#[instrument(skip_all, name = "data_channel_on_message", level = "trace")]
fn data_channel_on_message(
    data_channel_label: String,
) -> Result<OnMessageHdlrFn, SpanErr<PhonexError>> {
    Ok(Box::new(move |msg: DataChannelMessage| {
        let msg_str = String::from_utf8(msg.data.to_vec())
            .map_err(PhonexError::ConvertByteToString)
            .unwrap();
        println!("Message from DataChannel '{data_channel_label}': '{msg_str}'");
        Box::pin(async {})
    }))
}
