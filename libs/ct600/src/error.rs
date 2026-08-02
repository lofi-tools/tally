use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum Ct600Error {
    #[snafu(display("{message}"))]
    XmlError { message: String },

    #[snafu(display("{message}"))]
    ConfigError { message: String },

    #[snafu(display("{message}"))]
    C14nError { message: String },
}

pub type Result<T> = std::result::Result<T, Ct600Error>;
