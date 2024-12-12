use std::string::FromUtf8Error;

use thiserror::Error;
use tracing_subscriber::util::TryInitError;

#[derive(Error, Debug)]
pub enum PhonexError {
    #[error("failed to initialize tracing subscriber. {0}")]
    InitializeTracingSubscriber(TryInitError),
    #[error("failed to send http request: {0}")]
    SendHTTPRequest(reqwest::Error),
    #[error("failed to create new tcp listener: {0}")]
    BuildTcpListener(std::io::Error),
    #[error("failed to serve http: {0}")]
    ServeHTTP(std::io::Error),
    #[error("failed to initialize registry: {0}")]
    InitializeRegistry(webrtc::Error),
    #[error("failed to create new peer connection: {0}")]
    CreateNewPeerConnection(webrtc::Error),
    #[error("failed to close peer connection: {0}")]
    ClosePeerConnection(webrtc::Error),
    #[error("failed to convert to string: {0}")]
    ConvertByteToString(FromUtf8Error),
}
