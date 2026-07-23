use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache miss: {0}")]
    CacheMiss(String),
    #[error("corrupt cache: {0}")]
    Corrupt(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("compression error: {0}")]
    Compress(String),
}

pub type Result<T> = std::result::Result<T, Error>;
