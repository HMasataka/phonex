use std::string::FromUtf8Error;

use thiserror::Error;
use tracing_subscriber::util::TryInitError;

#[derive(Error, Debug)]
pub enum PhonexError {
    #[error("failed to initialize tracing subscriber. {0}")]
    InitializeTracingSubscriber(TryInitError),
    #[error("failed to send message: {0}")]
    SendMessage(webrtc::Error),
    #[error("failed to create new tcp listener: {0}")]
    BuildTcpListener(std::io::Error),
    #[error("failed to serve http: {0}")]
    ServeHTTP(std::io::Error),
    #[error("failed to initialize registry: {0}")]
    InitializeRegistry(webrtc::Error),
    #[error("failed to create new peer connection: {0}")]
    CreateNewPeerConnection(webrtc::Error),
    #[error("failed to create new data channel: {0}")]
    CreateNewDataChannel(webrtc::Error),
    #[error("failed to create new offer: {0}")]
    CreateNewOffer(webrtc::Error),
    #[error("failed to set remote description: {0}")]
    SetRemoteDescription(webrtc::Error),
    #[error("failed to set local description: {0}")]
    SetLocalDescription(webrtc::Error),
    #[error("failed to convert to string: {0}")]
    ConvertByteToString(FromUtf8Error),
    #[error("failed to add ice candidate: {0}")]
    AddIceCandidate(webrtc::Error),
    #[error("failed to convert json: {0}")]
    ConvertToJson(webrtc::Error),
}
