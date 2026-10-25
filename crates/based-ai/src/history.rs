//! Saved conversations: one JSON file per chat in a directory. Blocking I/O.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::{Message, Role};

const TITLE_CHARS: usize = 48;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    pub id: String,
    pub title: String,
    /// Unix milliseconds of the last save.
    pub updated: u64,
    pub messages: Vec<Message>,
    /// Connection label each tool call ran against, by call id.
    #[serde(default)]
    pub targets: HashMap<String, String>,
}

/// What the history list needs; parsing skips the messages.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ChatMeta {
    pub id: String,
    pub title: String,
    pub updated: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// A fresh, filename-safe id.
pub fn new_id() -> String {
    format!("{:x}", now_ms())
}

/// First line of the first user message, clipped.
pub fn title_of(messages: &[Message]) -> String {
    let first = messages
        .iter()
        .find(|m| m.role == Role::User)
        .and_then(|m| m.content.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("New chat")
        .trim();
    if first.chars().count() > TITLE_CHARS {
        format!("{}...", first.chars().take(TITLE_CHARS).collect::<String>())
    } else {
        first.to_owned()
    }
}

impl Chat {
    pub fn new(id: String, messages: Vec<Message>, targets: HashMap<String, String>) -> Self {
        Self {
            title: title_of(&messages),
            id,
            updated: now_ms(),
            messages,
            targets,
        }
    }

    pub fn meta(&self) -> ChatMeta {
        ChatMeta {
            id: self.id.clone(),
            title: self.title.clone(),
            updated: self.updated,
        }
    }
}

fn path(dir: &Path, id: &str) -> io::Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad chat id"));
    }
    Ok(dir.join(format!("{id}.json")))
}

pub fn save(dir: &Path, chat: &Chat) -> io::Result<()> {
    let target = path(dir, &chat.id)?;
    fs::create_dir_all(dir)?;
    let tmp = target.with_extension("tmp");
    fs::write(&tmp, serde_json::to_vec(chat)?)?;
    fs::rename(tmp, target)
}

pub fn load(dir: &Path, id: &str) -> io::Result<Chat> {
    Ok(serde_json::from_slice(&fs::read(path(dir, id)?)?)?)
}

pub fn delete(dir: &Path, id: &str) -> io::Result<()> {
    match fs::remove_file(path(dir, id)?) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Saved chats, newest first. Unreadable files are skipped.
pub fn list(dir: &Path) -> Vec<ChatMeta> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut chats: Vec<ChatMeta> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok())
        .collect();
    chats.sort_by_key(|c| Reverse(c.updated));
    chats
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::tools::{ToolCall, ToolResult};

    fn chat(id: &str, first: &str) -> Chat {
        let call = ToolCall {
            id: "c1".into(),
            name: "list_tables".into(),
            args: json!({}),
        };
        let messages = vec![
            Message::user(first),
            Message::assistant_calls("", vec![call.clone()]),
            Message::tool(&call, ToolResult::ok("users")),
            Message::assistant("done"),
        ];
        Chat::new(
            id.into(),
            messages,
            HashMap::from([("c1".into(), "prod".into())]),
        )
    }

    #[test]
    fn round_trips_with_tool_turns() {
        let dir = tempfile::tempdir().unwrap();
        let saved = chat("a1", "how many users?");
        save(dir.path(), &saved).unwrap();
        assert_eq!(load(dir.path(), "a1").unwrap(), saved);
    }

    #[test]
    fn lists_newest_first_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        let mut old = chat("a1", "old");
        old.updated = 1;
        let mut new = chat("b2", "new");
        new.updated = 2;
        save(dir.path(), &old).unwrap();
        save(dir.path(), &new).unwrap();
        let titles: Vec<_> = list(dir.path()).into_iter().map(|c| c.title).collect();
        assert_eq!(titles, ["new", "old"]);
        delete(dir.path(), "b2").unwrap();
        delete(dir.path(), "b2").unwrap();
        assert_eq!(list(dir.path()).len(), 1);
    }

    #[test]
    fn titles_are_clipped_and_ids_checked() {
        assert_eq!(title_of(&[Message::user("\n  hi there\nmore")]), "hi there");
        assert!(title_of(&[Message::user("x".repeat(100))]).ends_with("..."));
        assert!(load(Path::new("/tmp"), "../etc").is_err());
    }
}
