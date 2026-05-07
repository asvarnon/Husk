use thiserror::Error;

#[derive(Error, Debug)]
pub enum HomelabError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Endpoint error: {0}")]
    EndpointError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Task error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, HomelabError>;
