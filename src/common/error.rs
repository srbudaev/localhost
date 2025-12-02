use std::fmt;

#[derive(Debug)]
pub enum ServerError {
    IoError(std::io::Error),
    ConfigError(String),
    ParseError(String),
    NetworkError(String),
    HttpError(String),
    CgiError(String),
    TimeoutError(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::IoError(e) => write!(f, "IO error: {}", e),
            ServerError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            ServerError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ServerError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ServerError::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            ServerError::CgiError(msg) => write!(f, "CGI error: {}", msg),
            ServerError::TimeoutError(msg) => write!(f, "Timeout error: {}", msg),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<std::io::Error> for ServerError {
    fn from(err: std::io::Error) -> Self {
        ServerError::IoError(err)
    }
}

pub type Result<T> = std::result::Result<T, ServerError>;

