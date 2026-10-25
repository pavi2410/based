use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiErrorKind {
    /// Missing or rejected API key.
    Auth,
    RateLimited,
    /// Network failure or provider unreachable (including a stopped Ollama).
    Network,
    /// Provider rejected the request (bad model, context too long, ...).
    BadRequest,
    /// Malformed stream or unexpected payload.
    Protocol,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiError {
    pub kind: AiErrorKind,
    pub message: String,
}

impl AiError {
    pub fn new(kind: AiErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn protocol(message: impl Into<String>) -> Self {
        Self::new(AiErrorKind::Protocol, message)
    }

    /// Maps an HTTP status from a provider to an error kind.
    pub fn from_status(status: u16, body: impl Into<String>) -> Self {
        let kind = match status {
            401 | 403 => AiErrorKind::Auth,
            429 => AiErrorKind::RateLimited,
            400..=499 => AiErrorKind::BadRequest,
            500..=599 => AiErrorKind::Network,
            _ => AiErrorKind::Other,
        };
        Self::new(kind, format!("HTTP {status}: {}", body.into()))
    }
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for AiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping() {
        assert_eq!(AiError::from_status(401, "").kind, AiErrorKind::Auth);
        assert_eq!(AiError::from_status(429, "").kind, AiErrorKind::RateLimited);
        assert_eq!(AiError::from_status(400, "").kind, AiErrorKind::BadRequest);
        assert_eq!(AiError::from_status(503, "").kind, AiErrorKind::Network);
    }
}
