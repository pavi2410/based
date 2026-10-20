//! Redis configuration, typed key access, and a raw command runner (no UI).

pub mod client;
pub mod config;
pub mod console;
pub mod keys;
pub mod mutate;
pub mod value;

pub use client::{Conn, connect, database_count, ping, server_version};
pub use config::{RedisConfig, redis_cli_command, redis_url, url_db, url_with_db};
pub use console::{exec, format_reply, is_dangerous, tokenize};
pub use keys::{KeyMeta, KeyType, key_meta, scan_keys};
pub use mutate::{add_entry, delete_key, remove_entry, set_string, set_ttl};
pub use value::{Entry, KeyValue, PAGE, read_value};
