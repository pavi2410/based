//! SQL autocomplete for SQLite query editors.

use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use gpui_kit::component::Rope;
use gpui_kit::component::input::{CompletionProvider, RopeExt};
use gpui_kit::{App, AppContext as _, Task, Window};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Range, TextEdit,
};

use super::schema_cache::{ObjectKind, SchemaCache, quote_ident};

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
    "ILIKE",
    "RETURNING",
    "USING",
    "ANALYZE",
    "TRUNCATE",
    "CONFLICT",
    "DO",
    "NOTHING",
    "LATERAL",
    "OVER",
    "PARTITION",
    "FILTER",
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

/// Identifier token ending at `end` (byte offset into `text`): returns its start.
/// Quoted segments (`"my table"`) may contain spaces and dots.
fn token_start(text: &str, end: usize) -> usize {
    let line_start = text[..end].rfind('\n').map_or(0, |i| i + 1);
    let (mut start, mut inq) = (line_start, false);
    for (i, c) in text[line_start..end].char_indices() {
        match c {
            '"' => inq = !inq,
            _ if inq => {}
            c if c.is_alphanumeric() || matches!(c, '_' | '.' | '$') => {}
            c => start = line_start + i + c.len_utf8(),
        }
    }
    start
}

/// Byte index of the last `.` outside quotes.
fn last_dot(token: &str) -> Option<usize> {
    let (mut dot, mut inq) = (None, false);
    for (i, c) in token.char_indices() {
        match c {
            '"' => inq = !inq,
            '.' if !inq => dot = Some(i),
            _ => {}
        }
    }
    dot
}

impl CompletionProvider for SqlCompletionProvider {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let text = rope.to_string();
        let mut end = offset.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let start = token_start(&text, end);
        let token = text[start..end].to_string();
        if token.is_empty() {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        }

        // Replace only the part being typed (after the last dot, if any).
        let (qualifier, prefix, from) = match last_dot(&token) {
            Some(d) => (
                Some(token[..d].to_string()),
                token[d + 1..].to_string(),
                start + d + 1,
            ),
            None => (None, token, start),
        };
        let range = Range::new(rope.offset_to_position(from), rope.offset_to_position(end));

        let cache = self.cache.clone();
        cx.background_spawn(async move {
            let items = match qualifier {
                Some(table) => column_items(&cache, &table, &prefix, &range),
                None => schema_and_keyword_items(&cache, &prefix, &range),
            };
            Ok(CompletionResponse::Array(items))
        })
    }

    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        !new_text.is_empty()
            && new_text
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '"' | ' ' | '$'))
    }
}

fn schema_and_keyword_items(
    cache: &SchemaCache,
    query: &str,
    range: &Range,
) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = cache
        .complete(query)
        .into_iter()
        .map(|obj| {
            let kind = if obj.kind == ObjectKind::View {
                "view"
            } else {
                "table"
            };
            item(
                &obj.display(),
                CompletionItemKind::CLASS,
                Some(kind),
                range,
                obj.insert_text(),
            )
        })
        .collect();

    // Keywords and functions are never quoted, so a leading `"` means identifiers only.
    if !query.starts_with('"') {
        let lower = query.to_lowercase();
        let words = SQL_KEYWORDS
            .iter()
            .map(|k| (k, CompletionItemKind::KEYWORD, None))
            .chain(
                SQL_FUNCTIONS
                    .iter()
                    .map(|f| (f, CompletionItemKind::FUNCTION, Some("()"))),
            );
        for (w, kind, detail) in words {
            if w.to_lowercase().starts_with(&lower)
                && !items.iter().any(|i| i.label.eq_ignore_ascii_case(w))
            {
                items.push(item(w, kind, detail, range, (*w).to_string()));
            }
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
    range: &Range,
) -> Vec<CompletionItem> {
    cache
        .complete_columns(table, col_prefix)
        .into_iter()
        .map(|col| {
            item(
                &col.name,
                CompletionItemKind::FIELD,
                Some(&col.data_type),
                range,
                quote_ident(&col.name),
            )
        })
        .collect()
}

fn item(
    label: &str,
    kind: CompletionItemKind,
    detail: Option<&str>,
    range: &Range,
    new_text: String,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: detail.map(str::to_string),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: *range,
            new_text,
        })),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_spans_quoted_spaces() {
        let t = "select * from \"my ta";
        assert_eq!(&t[token_start(t, t.len())..], "\"my ta");
        let t = "select a ";
        assert_eq!(token_start(t, t.len()), t.len());
    }

    #[test]
    fn last_dot_ignores_quoted() {
        assert_eq!(last_dot("\"a.b\".c"), Some(5));
        assert_eq!(last_dot("\"a.b"), None);
    }
}
