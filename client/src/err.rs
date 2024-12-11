use thiserror::Error;
use tracing_subscriber::util::TryInitError;

#[derive(Error, Debug)]
pub enum PhonexError {
    #[error("failed to initialize tracing subscriber. {0}")]
    InitializeTracingSubscriber(TryInitError),
    #[error("failed to send message: {0}")]
    SendMessage(webrtc::Error),
    #[error("failed to initialize registry: {0}")]
    InitializeRegistry(webrtc::Error),
    #[error("failed to create new peer connection: {0}")]
    CreateNewPeerConnection(webrtc::Error),
}
