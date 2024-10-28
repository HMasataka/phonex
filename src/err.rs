use thiserror::Error;
use tracing_subscriber::util::TryInitError;

#[derive(Error, Debug)]
pub enum PhonexError {
    #[error("failed to initialize tracing subscriber. {0}")]
    InitializeTracingSubscriber(TryInitError),
}
