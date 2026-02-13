#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("config error: {0}")]
    Config(String),
    #[error("request failed: {0}")]
    Request(String),
    #[error("auth error: {0}")]
    Auth(String),
    #[error("request timeout: {0}")]
    Timeout(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("bridge error: {0}")]
    Bridge(String),
    #[error("runtime error: {0}")]
    Runtime(String),
}

impl From<reqwest::Error> for EmbeddingError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() {
            return Self::Timeout(value.to_string());
        }
        Self::Request(value.to_string())
    }
}

impl From<std::io::Error> for EmbeddingError {
    fn from(value: std::io::Error) -> Self {
        Self::Runtime(value.to_string())
    }
}
