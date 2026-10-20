use anyhow::{Result, bail};

use crate::client::Conn;
use crate::keys::KeyType;

pub async fn delete_key(conn: &mut Conn, key: &str) -> Result<()> {
    redis::cmd("DEL").arg(key).query_async::<()>(conn).await?;
    Ok(())
}

/// `Some(secs)` sets an expiry; `None` removes it.
pub async fn set_ttl(conn: &mut Conn, key: &str, secs: Option<i64>) -> Result<()> {
    match secs {
        Some(s) => {
            redis::cmd("EXPIRE")
                .arg(key)
                .arg(s)
                .query_async::<()>(conn)
                .await?
        }
        None => {
            redis::cmd("PERSIST")
                .arg(key)
                .query_async::<()>(conn)
                .await?
        }
    }
    Ok(())
}

/// Overwrite a string value, keeping any existing TTL.
pub async fn set_string(conn: &mut Conn, key: &str, value: &str) -> Result<()> {
    redis::cmd("SET")
        .arg(key)
        .arg(value)
        .arg("KEEPTTL")
        .query_async::<()>(conn)
        .await?;
    Ok(())
}

/// Add one entry from free text: `field value` (hash), `score member` (zset), or the raw item.
pub async fn add_entry(conn: &mut Conn, key: &str, kind: KeyType, input: &str) -> Result<()> {
    let input = input.trim();
    if input.is_empty() {
        bail!("nothing to add");
    }
    let cmd = match kind {
        KeyType::Hash => {
            let (field, value) = split_pair(input)?;
            let mut c = redis::cmd("HSET");
            c.arg(key).arg(field).arg(value);
            c
        }
        KeyType::ZSet => {
            let (score, member) = split_pair(input)?;
            let score: f64 = score
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid score"))?;
            let mut c = redis::cmd("ZADD");
            c.arg(key).arg(score).arg(member);
            c
        }
        KeyType::List => {
            let mut c = redis::cmd("RPUSH");
            c.arg(key).arg(input);
            c
        }
        KeyType::Set => {
            let mut c = redis::cmd("SADD");
            c.arg(key).arg(input);
            c
        }
        _ => bail!("cannot add entries to a {} key", kind.label()),
    };
    cmd.query_async::<()>(conn).await?;
    Ok(())
}

/// Remove the entry identified by `id` (hash field, or list/set/zset member).
pub async fn remove_entry(conn: &mut Conn, key: &str, kind: KeyType, id: &str) -> Result<()> {
    let cmd = match kind {
        KeyType::Hash => {
            let mut c = redis::cmd("HDEL");
            c.arg(key).arg(id);
            c
        }
        KeyType::List => {
            let mut c = redis::cmd("LREM");
            c.arg(key).arg(1).arg(id);
            c
        }
        KeyType::Set => {
            let mut c = redis::cmd("SREM");
            c.arg(key).arg(id);
            c
        }
        KeyType::ZSet => {
            let mut c = redis::cmd("ZREM");
            c.arg(key).arg(id);
            c
        }
        _ => bail!("cannot remove entries from a {} key", kind.label()),
    };
    cmd.query_async::<()>(conn).await?;
    Ok(())
}

fn split_pair(input: &str) -> Result<(&str, &str)> {
    match input.split_once(char::is_whitespace) {
        Some((a, b)) if !b.trim().is_empty() => Ok((a, b.trim())),
        _ => bail!("expected two values separated by a space"),
    }
}

#[cfg(test)]
mod tests {
    use super::split_pair;

    #[test]
    fn splits_on_first_whitespace_and_keeps_rest() {
        assert_eq!(
            split_pair("name Ada Lovelace").unwrap(),
            ("name", "Ada Lovelace")
        );
    }

    #[test]
    fn rejects_single_token() {
        assert!(split_pair("name").is_err());
        assert!(split_pair("name   ").is_err());
    }
}
