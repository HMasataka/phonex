use thiserror::Error;
use tracing_subscriber::util::TryInitError;

#[derive(Error, Debug)]
pub enum PhonexError {
    #[error("failed to initialize tracing subscriber. {0}")]
    InitializeTracingSubscriber(TryInitError),
    #[error("failed to send message: {0}")]
    FailedToSend(webrtc::Error),
}
