use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("translation `{0}` is not installed")]
    NotInstalled(String),

    #[error("translation `{0}` is unknown")]
    UnknownTranslation(String),

    #[error("invalid book number {0} (expected 1..=66)")]
    InvalidBookNumber(u8),

    #[error("could not locate user data directory")]
    NoDataDir,

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("xml: {0}")]
    Xml(String),

    #[error("encode/decode: {0}")]
    Bincode(String),

    #[error("toml: {0}")]
    Toml(String),

    #[error("network: {0}")]
    Http(String),

    #[error("reference: {0}")]
    Reference(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
