use anyhow::Result;
use redis::aio::ConnectionManager;

/// Auto-reconnecting, cheaply cloneable connection handle.
pub type Conn = ConnectionManager;

pub async fn connect(url: &str) -> Result<Conn> {
    let client = redis::Client::open(url)?;
    Ok(client.get_connection_manager().await?)
}

pub async fn ping(conn: &mut Conn) -> Result<()> {
    redis::cmd("PING").query_async::<()>(conn).await?;
    Ok(())
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
