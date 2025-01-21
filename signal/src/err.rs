use std::string::FromUtf8Error;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio_tungstenite::tungstenite;
use tracing_subscriber::util::TryInitError;

use crate::r#match::{MatchRequest, MatchResponse};

#[derive(Error, Debug)]
pub enum SignalError {
    #[error("initialize tracing subscriber error. {0}")]
    InitializeTracingSubscriber(TryInitError),
    #[error("failed to create new tcp listener: {0}")]
    BuildTcpListener(std::io::Error),
    #[error("deserialize to string error: {0}")]
    DeserializeJSONError(serde_json::Error),
    #[error("serialize to json error: {0}")]
    SerializeToJSONError(serde_json::Error),
    #[error("convert to string error: {0}")]
    ConvertToStringError(FromUtf8Error),
    #[error("failed to write websocket: {0}")]
    WriteWebsocket(axum::Error),
    #[error("failed to send match request: {0}")]
    SendMatchRequest(SendError<MatchRequest>),
    #[error("failed to send match response: {0}")]
    SendMatchResponse(SendError<MatchResponse>),
    #[error("register not found")]
    RegisterNotFound(),
    #[error("failed to serve http: {0}")]
    ServeHTTP(std::io::Error),
    #[error("failed to send message: {0}")]
    SendMessage(webrtc::Error),
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
    #[error("deserialize to string error: {0}")]
    FromJSONError(serde_json::Error),
    #[error("serialize to json error: {0}")]
    ToJSONError(serde_json::Error),
    #[error("failed to send websocket message: {0}")]
    SendWebSocketMessage(tungstenite::Error),
}

impl IntoResponse for SignalError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self),
        )
            .into_response()
    }
}
