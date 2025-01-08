use std::string::FromUtf8Error;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tracing_subscriber::util::TryInitError;

use crate::r#match::{MatchRequest, MatchResponse};

#[derive(Error, Debug)]
pub enum PhonexError {
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
}

impl IntoResponse for PhonexError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self),
        )
            .into_response()
    }
}
