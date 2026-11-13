use anyhow::Result;
use based_core::{ConnectError, ConnectionErrorCategory as C};
use redis::aio::ConnectionManager;
use redis::{ErrorKind, RedisError};

/// Auto-reconnecting, cheaply cloneable connection handle.
pub type Conn = ConnectionManager;

pub async fn connect(url: &str) -> Result<Conn, ConnectError> {
    let client = redis::Client::open(url).map_err(connect_error)?;
    client.get_connection_manager().await.map_err(connect_error)
}

pub async fn ping(conn: &mut Conn) -> Result<(), ConnectError> {
    redis::cmd("PING")
        .query_async::<()>(conn)
        .await
        .map_err(connect_error)
}

/// Classify a failed connect by the driver's error kind.
pub fn connect_error(err: RedisError) -> ConnectError {
    let category = if err.kind() == ErrorKind::AuthenticationFailed {
        C::AuthFailed
    } else if err.kind() == ErrorKind::InvalidClientConfig {
        C::UriParse
    } else if err.is_timeout() {
        C::NetTimeout
    } else if err.is_connection_refusal() || err.is_io_error() {
        C::NetUnreachable
    } else {
        C::ServerError
    };
    ConnectError::new(category, err)
}

/// Number of logical databases (`CONFIG GET databases`); 16 when the server hides it.
pub async fn database_count(conn: &mut Conn) -> u32 {
    let reply: Result<Vec<String>, _> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("databases")
        .query_async(conn)
        .await;
    reply
        .ok()
        .and_then(|v| v.get(1).and_then(|n| n.parse().ok()))
        .unwrap_or(16)
}

/// `redis_version` from `INFO server`, if the server exposes it.
pub async fn server_version(conn: &mut Conn) -> Option<String> {
    let info: String = redis::cmd("INFO")
        .arg("server")
        .query_async(conn)
        .await
        .ok()?;
    parse_version(&info)
}

fn parse_version(info: &str) -> Option<String> {
    info.lines()
        .find_map(|l| l.strip_prefix("redis_version:"))
        .map(|v| v.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_version;

    #[test]
    fn parses_version_line() {
        let info = "# Server\r\nredis_version:7.4.1\r\nos:Linux\r\n";
        assert_eq!(parse_version(info).as_deref(), Some("7.4.1"));
    }

    #[test]
    fn missing_version_is_none() {
        assert_eq!(parse_version("# Server\r\n"), None);
    }
}

#[cfg(test)]
mod connect_error_tests {
    use super::*;

    #[test]
    fn bad_urls_and_refused_ports() {
        let bad = redis::Client::open("notredis://x").unwrap_err();
        assert_eq!(connect_error(bad).category, C::UriParse);
    }

    #[tokio::test]
    async fn refused_port_is_unreachable() {
        let Err(e) = connect("redis://127.0.0.1:1").await else {
            panic!("port 1 should refuse");
        };
        assert_eq!(e.category, C::NetUnreachable, "{e}");
    }
}
