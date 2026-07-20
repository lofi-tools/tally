use thiserror::Error;

#[derive(Debug, Error)]
pub enum Ct600Error {
    #[error("{0}")]
    XmlError(String),

    #[error("{0}")]
    ConfigError(String),

    #[error("{0}")]
    C14nError(String),
}

pub type Result<T> = std::result::Result<T, Ct600Error>;
