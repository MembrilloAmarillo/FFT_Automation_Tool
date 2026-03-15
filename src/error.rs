use thiserror::Error;

#[derive(Debug, Error)]
pub enum YamcsTcError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("command error: {0}")]
    Command(String),

    #[error("telemetry verification failed: {0}")]
    Verification(String),
}

pub type Result<T> = std::result::Result<T, YamcsTcError>;
