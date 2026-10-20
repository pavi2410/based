use anyhow::Result;

use crate::client::Conn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    String,
    Hash,
    List,
    Set,
    ZSet,
    Stream,
    Other,
}

impl KeyType {
    pub fn parse(s: &str) -> Self {
        match s {
            "string" => Self::String,
            "hash" => Self::Hash,
            "list" => Self::List,
            "set" => Self::Set,
            "zset" => Self::ZSet,
            "stream" => Self::Stream,
            _ => Self::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Hash => "hash",
            Self::List => "list",
            Self::Set => "set",
            Self::ZSet => "zset",
            Self::Stream => "stream",
            Self::Other => "other",
        }
    }

    fn len_command(self) -> Option<&'static str> {
        match self {
            Self::String => Some("STRLEN"),
            Self::Hash => Some("HLEN"),
            Self::List => Some("LLEN"),
            Self::Set => Some("SCARD"),
            Self::ZSet => Some("ZCARD"),
            Self::Stream => Some("XLEN"),
            Self::Other => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyMeta {
    pub kind: Option<KeyType>,
    /// Seconds to live; `None` when the key has no expiry.
    pub ttl: Option<i64>,
    /// Element count (bytes for strings).
    pub len: Option<i64>,
    /// `MEMORY USAGE` in bytes.
    pub memory: Option<i64>,
}

/// Cursor-iterate matching keys until `limit` is reached. Never uses `KEYS`.
pub async fn scan_keys(conn: &mut Conn, pattern: &str, limit: usize) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut cursor: u64 = 0;
    loop {
        let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(500)
            .query_async(conn)
            .await?;
        out.extend(batch);
        cursor = next;
        if cursor == 0 || out.len() >= limit {
            break;
        }
    }
    out.truncate(limit);
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// Type, TTL, length and memory in one pipelined round trip. `kind` is `None` when the key is gone.
pub async fn key_meta(conn: &mut Conn, key: &str) -> Result<KeyMeta> {
    let (ty, ttl): (String, i64) = redis::pipe()
        .cmd("TYPE")
        .arg(key)
        .cmd("TTL")
        .arg(key)
        .query_async(conn)
        .await?;
    if ty == "none" {
        return Ok(KeyMeta {
            kind: None,
            ttl: None,
            len: None,
            memory: None,
        });
    }
    let kind = KeyType::parse(&ty);
    let len = match kind.len_command() {
        Some(cmd) => redis::cmd(cmd).arg(key).query_async(conn).await.ok(),
        None => None,
    };
    let memory = redis::cmd("MEMORY")
        .arg("USAGE")
        .arg(key)
        .query_async::<Option<i64>>(conn)
        .await
        .ok()
        .flatten();
    Ok(KeyMeta {
        kind: Some(kind),
        ttl: (ttl >= 0).then_some(ttl),
        len,
        memory,
    })
}

#[cfg(test)]
mod tests {
    use super::KeyType;

    #[test]
    fn parses_type_names() {
        assert_eq!(KeyType::parse("zset"), KeyType::ZSet);
        assert_eq!(KeyType::parse("ReJSON-RL"), KeyType::Other);
    }

    #[test]
    fn label_round_trips() {
        for t in [
            KeyType::String,
            KeyType::Hash,
            KeyType::List,
            KeyType::Set,
            KeyType::ZSet,
            KeyType::Stream,
        ] {
            assert_eq!(KeyType::parse(t.label()), t);
        }
    }
}
