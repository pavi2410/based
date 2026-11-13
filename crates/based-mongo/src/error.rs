//! Driver errors → typed connect errors, by error kind and server code.

use based_core::{ConnectError, ConnectionErrorCategory as C};
use mongodb::error::{Error, ErrorKind};

/// `AuthenticationFailed` and `Unauthorized` server codes.
const AUTH_CODES: [i32; 2] = [18, 13];

/// Classify a failed connect.
pub fn connect_error(err: Error) -> ConnectError {
    let category = match err.kind.as_ref() {
        ErrorKind::Authentication { .. } => C::AuthFailed,
        ErrorKind::Command(cmd) if AUTH_CODES.contains(&cmd.code) => C::AuthFailed,
        ErrorKind::ServerSelection { .. } | ErrorKind::DnsResolve { .. } => C::NetUnreachable,
        ErrorKind::Io(io) => return ConnectError::io(io),
        ErrorKind::InvalidArgument { .. } => C::UriParse,
        ErrorKind::InvalidTlsConfig { .. } => C::TlsRejected,
        _ => C::ServerError,
    };
    ConnectError::new(category, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_client_options;

    #[tokio::test]
    async fn a_malformed_uri_is_a_uri_error() {
        let err = parse_client_options("mongodb://host:notaport")
            .await
            .unwrap_err();
        assert_eq!(connect_error(err).category, C::UriParse);
    }
}
