use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("IO error: {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON error: {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("{0}")]
    Message(String),

    #[error("proxy error: {0}")]
    Proxy(String),
}

impl AppError {
    pub fn io(path: impl AsRef<Path>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.as_ref().display().to_string(),
            source,
        }
    }

    pub fn json(path: impl AsRef<Path>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.as_ref().display().to_string(),
            source,
        }
    }

    pub fn proxy(msg: String) -> Self {
        Self::Proxy(msg)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
