//! SQL autocomplete for SQLite query editors.

use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use gpui::{AppContext as _, Context, Task, Window};
use gpui_component::Rope;
use gpui_component::input::{CompletionProvider, InputState, RopeExt};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    TextEdit,
};

use super::schema_cache::SchemaCache;

const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "CROSS",
    "ON",
    "AS",
    "AND",
    "OR",
    "NOT",
    "IN",
    "IS",
    "NULL",
    "LIKE",
    "GLOB",
    "BETWEEN",
    "EXISTS",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "GROUP",
    "BY",
    "HAVING",
    "ORDER",
    "LIMIT",
    "OFFSET",
    "UNION",
    "ALL",
    "DISTINCT",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "TABLE",
    "VIEW",
    "INDEX",
    "IF",
    "EXISTS",
    "DROP",
    "ALTER",
    "PRAGMA",
    "EXPLAIN",
    "QUERY",
    "PLAN",
    "WITH",
    "RECURSIVE",
    "ASC",
    "DESC",
    "NULLS",
    "FIRST",
    "LAST",
];

const SQL_FUNCTIONS: &[&str] = &[
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "COALESCE",
    "NULLIF",
    "LENGTH",
    "LOWER",
    "UPPER",
    "TRIM",
    "SUBSTR",
    "REPLACE",
    "ABS",
    "ROUND",
    "CAST",
    "TYPEOF",
    "JSON",
    "MATCH",
    "snippet",
    "highlight",
    "rank",
    "bm25",
];

pub struct SqlCompletionProvider {
    cache: Arc<SchemaCache>,
}

impl SqlCompletionProvider {
    pub fn new(cache: Arc<SchemaCache>) -> Rc<Self> {
        Rc::new(Self { cache })
    }
}

impl CompletionProvider for SqlCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        trigger: CompletionContext,
        _: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<CompletionResponse>> {
        let query = trigger.trigger_character.unwrap_or_default();
        if query.is_empty() {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        }

        let start = offset.saturating_sub(query.len());
        let replace_range = lsp_types::Range::new(
            rope.offset_to_position(start),
            rope.offset_to_position(offset),
        );

        let cache = self.cache.clone();
        cx.background_spawn(async move {
            let items = if let Some((table, col_prefix)) = query.rsplit_once('.') {
                column_items(&cache, table, col_prefix, &query, &replace_range)
            } else {
                schema_and_keyword_items(&cache, &query, &replace_range)
            };
            Ok(CompletionResponse::Array(items))
        })
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        !new_text.is_empty()
            && new_text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    }
}

fn schema_and_keyword_items(
    cache: &SchemaCache,
    query: &str,
    replace_range: &lsp_types::Range,
) -> Vec<CompletionItem> {
    let lower = query.to_lowercase();
    let mut items = Vec::new();

    for obj in cache.complete(query) {
        items.push(completion_item(
            obj.label.as_str(),
            CompletionItemKind::CLASS,
            Some(obj.kind_label()),
            replace_range,
        ));
    }

    for kw in SQL_KEYWORDS {
        if kw.to_lowercase().starts_with(&lower)
            && !items.iter().any(|i| i.label.eq_ignore_ascii_case(kw))
        {
            items.push(completion_item(
                kw,
                CompletionItemKind::KEYWORD,
                None,
                replace_range,
            ));
        }
    }

    for func in SQL_FUNCTIONS {
        if func.to_lowercase().starts_with(&lower)
            && !items.iter().any(|i| i.label.eq_ignore_ascii_case(func))
        {
            items.push(completion_item(
                func,
                CompletionItemKind::FUNCTION,
                Some("()"),
                replace_range,
            ));
        }
    }

    items.sort_by_key(|a| a.label.to_lowercase());
    items.truncate(50);
    items
}

fn column_items(
    cache: &SchemaCache,
    table: &str,
    col_prefix: &str,
    full_query: &str,
    replace_range: &lsp_types::Range,
) -> Vec<CompletionItem> {
    let table_prefix = table.trim();
    cache
        .complete_columns(table_prefix, col_prefix)
        .into_iter()
        .map(|col| {
            let insert = format!("{table_prefix}.{}", col.name);
            completion_item_with_text(
                col.name.as_str(),
                CompletionItemKind::FIELD,
                Some(col.data_type.as_str()),
                replace_range,
                if col_prefix.is_empty() {
                    insert
                } else {
                    full_query.replacen(col_prefix, &col.name, 1)
                },
            )
        })
        .collect()
}

fn completion_item(
    label: &str,
    kind: CompletionItemKind,
    detail: Option<&str>,
    replace_range: &lsp_types::Range,
) -> CompletionItem {
    completion_item_with_text(label, kind, detail, replace_range, label.to_string())
}

fn completion_item_with_text(
    label: &str,
    kind: CompletionItemKind,
    detail: Option<&str>,
    replace_range: &lsp_types::Range,
    new_text: String,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: detail.map(str::to_string),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: *replace_range,
            new_text,
        })),
        ..Default::default()
    }
}

trait SchemaObjectExt {
    fn kind_label(&self) -> &'static str;
}

impl SchemaObjectExt for super::schema_cache::SchemaObject {
    fn kind_label(&self) -> &'static str {
        match self.kind {
            super::schema_cache::ObjectKind::View => "view",
            _ => "table",
        }
    }
}
