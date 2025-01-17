use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("network error: {0}")]
    NetworkError(#[from] std::io::Error),
    #[error("serde error: {0}")]
    SerdeError(#[from] serde_json::error::Error),
    #[error("codec error: {0}")]
    CodecError(#[from] tokio_util::codec::LinesCodecError),
}
