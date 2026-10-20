use anyhow::Result;

use crate::client::Conn;
use crate::keys::KeyType;

/// Max entries loaded for a collection key.
pub const PAGE: usize = 500;

/// One row of a collection. `id` is what [`crate::remove_entry`] needs to delete it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    /// Field, index, or score; empty for sets.
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone)]
pub enum KeyValue {
    Text(String),
    Entries(Vec<Entry>),
    Unsupported,
}

pub async fn read_value(conn: &mut Conn, key: &str, kind: KeyType) -> Result<KeyValue> {
    Ok(match kind {
        KeyType::String => {
            let bytes: Option<Vec<u8>> = redis::cmd("GET").arg(key).query_async(conn).await?;
            KeyValue::Text(String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned())
        }
        KeyType::Hash => {
            let flat = scan_flat(conn, "HSCAN", key).await?;
            KeyValue::Entries(pairs(flat, |f, v| Entry {
                id: f.clone(),
                left: f,
                right: v,
            }))
        }
        KeyType::Set => {
            let (_, members): (u64, Vec<String>) = redis::cmd("SSCAN")
                .arg(key)
                .arg(0)
                .arg("COUNT")
                .arg(PAGE)
                .query_async(conn)
                .await?;
            KeyValue::Entries(
                members
                    .into_iter()
                    .take(PAGE)
                    .map(|m| Entry {
                        id: m.clone(),
                        left: String::new(),
                        right: m,
                    })
                    .collect(),
            )
        }
        KeyType::List => {
            let items: Vec<String> = redis::cmd("LRANGE")
                .arg(key)
                .arg(0)
                .arg(PAGE - 1)
                .query_async(conn)
                .await?;
            KeyValue::Entries(
                items
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| Entry {
                        id: v.clone(),
                        left: i.to_string(),
                        right: v,
                    })
                    .collect(),
            )
        }
        KeyType::ZSet => {
            let flat: Vec<String> = redis::cmd("ZRANGE")
                .arg(key)
                .arg(0)
                .arg(PAGE - 1)
                .arg("WITHSCORES")
                .query_async(conn)
                .await?;
            KeyValue::Entries(pairs(flat, |m, s| Entry {
                id: m.clone(),
                left: s,
                right: m,
            }))
        }
        KeyType::Stream | KeyType::Other => KeyValue::Unsupported,
    })
}

/// Cursor-iterate a hash until [`PAGE`] flat items (field, value, ...) are collected.
async fn scan_flat(conn: &mut Conn, cmd: &str, key: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut cursor: u64 = 0;
    loop {
        let (next, batch): (u64, Vec<String>) = redis::cmd(cmd)
            .arg(key)
            .arg(cursor)
            .arg("COUNT")
            .arg(PAGE)
            .query_async(conn)
            .await?;
        out.extend(batch);
        cursor = next;
        if cursor == 0 || out.len() >= PAGE * 2 {
            break;
        }
    }
    out.truncate(PAGE * 2);
    Ok(out)
}

fn pairs(flat: Vec<String>, f: impl Fn(String, String) -> Entry) -> Vec<Entry> {
    flat.as_chunks::<2>()
        .0
        .iter()
        .map(|[a, b]| f(a.clone(), b.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_chunks_flat_arrays_and_drops_trailing_odd_item() {
        let flat = vec!["a".into(), "1".into(), "b".into(), "2".into(), "c".into()];
        let rows = pairs(flat, |k, v| Entry {
            id: k.clone(),
            left: k,
            right: v,
        });
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].left, "b");
        assert_eq!(rows[1].right, "2");
    }
}
