//! Redis console autocomplete: command names, then per-command option keywords.

use std::rc::Rc;

use anyhow::Result;
use gpui_kit::component::Rope;
use gpui_kit::component::input::{CompletionProvider, RopeExt};
use gpui_kit::{App, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Range, TextEdit,
};

const COMMANDS: &[&str] = &[
    "APPEND",
    "AUTH",
    "BITCOUNT",
    "BLPOP",
    "BRPOP",
    "CLIENT",
    "CONFIG",
    "COPY",
    "DBSIZE",
    "DECR",
    "DECRBY",
    "DEL",
    "DUMP",
    "ECHO",
    "EVAL",
    "EXISTS",
    "EXPIRE",
    "EXPIREAT",
    "FLUSHALL",
    "FLUSHDB",
    "GET",
    "GETDEL",
    "GETEX",
    "GETSET",
    "HDEL",
    "HEXISTS",
    "HGET",
    "HGETALL",
    "HINCRBY",
    "HKEYS",
    "HLEN",
    "HMGET",
    "HMSET",
    "HSCAN",
    "HSET",
    "HVALS",
    "INCR",
    "INCRBY",
    "INCRBYFLOAT",
    "INFO",
    "KEYS",
    "LINDEX",
    "LLEN",
    "LPOP",
    "LPUSH",
    "LRANGE",
    "LREM",
    "LSET",
    "LTRIM",
    "MGET",
    "MSET",
    "MULTI",
    "EXEC",
    "PERSIST",
    "PEXPIRE",
    "PING",
    "PTTL",
    "PUBLISH",
    "RENAME",
    "RPOP",
    "RPUSH",
    "SADD",
    "SCAN",
    "SCARD",
    "SELECT",
    "SET",
    "SETEX",
    "SETNX",
    "SISMEMBER",
    "SMEMBERS",
    "SPOP",
    "SREM",
    "SSCAN",
    "STRLEN",
    "TTL",
    "TYPE",
    "UNLINK",
    "XADD",
    "XLEN",
    "XRANGE",
    "XREAD",
    "ZADD",
    "ZCARD",
    "ZINCRBY",
    "ZRANGE",
    "ZRANGEBYSCORE",
    "ZREM",
    "ZREVRANGE",
    "ZSCORE",
    "ZSCAN",
];

/// Option keywords valid after a given command.
fn options(cmd: &str) -> &'static [&'static str] {
    match cmd {
        "SCAN" | "SSCAN" | "HSCAN" | "ZSCAN" => &["MATCH", "COUNT", "TYPE"],
        "SET" => &["EX", "PX", "EXAT", "PXAT", "NX", "XX", "KEEPTTL", "GET"],
        "EXPIRE" | "PEXPIRE" => &["NX", "XX", "GT", "LT"],
        "ZADD" => &["NX", "XX", "GT", "LT", "CH", "INCR"],
        "ZRANGE" | "ZREVRANGE" => &["WITHSCORES", "BYSCORE", "BYLEX", "REV", "LIMIT"],
        "ZRANGEBYSCORE" => &["WITHSCORES", "LIMIT"],
        "FLUSHDB" | "FLUSHALL" => &["ASYNC", "SYNC"],
        "CONFIG" => &["GET", "SET", "RESETSTAT", "REWRITE"],
        "CLIENT" => &["LIST", "KILL", "SETNAME", "GETNAME", "ID"],
        "INFO" => &[
            "server",
            "clients",
            "memory",
            "stats",
            "replication",
            "keyspace",
            "all",
        ],
        "XADD" => &["MAXLEN", "NOMKSTREAM", "MINID"],
        "GETEX" => &["EX", "PX", "EXAT", "PXAT", "PERSIST"],
        _ => &[],
    }
}

pub struct RedisCompletionProvider;

impl RedisCompletionProvider {
    pub fn new() -> Rc<Self> {
        Rc::new(Self)
    }
}

impl CompletionProvider for RedisCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        _: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let text = rope.to_string();
        let mut end = offset.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let line = text[..end].rsplit('\n').next().unwrap_or("");
        let word_start = line
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(i, c)| i + c.len_utf8());
        let (head, prefix) = line.split_at(word_start);
        let first = head.split_whitespace().next();

        let (pool, kind): (&[&str], _) = match first {
            None => (COMMANDS, CompletionItemKind::FUNCTION),
            Some(cmd) => (options(&cmd.to_uppercase()), CompletionItemKind::KEYWORD),
        };

        let (start, offset) = (end - prefix.len(), end);
        let range = Range::new(
            rope.offset_to_position(start),
            rope.offset_to_position(offset),
        );
        let lower = prefix.to_lowercase();
        let items = pool
            .iter()
            .filter(|c| c.to_lowercase().starts_with(&lower) && !c.eq_ignore_ascii_case(prefix))
            .take(50)
            .map(|c| CompletionItem {
                label: (*c).to_string(),
                kind: Some(kind),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: (*c).to_string(),
                })),
                ..Default::default()
            })
            .collect();
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        !new_text.is_empty()
            && new_text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}
