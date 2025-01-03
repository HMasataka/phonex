use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum PhonexError {
    #[error("failed to create new tcp listener: {0}")]
    BuildTcpListener(std::io::Error),
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
