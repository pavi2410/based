//! Read-only-ish SQL display with naive token coloring (keywords, strings, numbers, comments).

use gpui::{div, prelude::*, Context, Hsla, IntoElement, Render, Window};
use gpui_component::ActiveTheme;

#[derive(Clone, PartialEq, Eq)]
pub enum EditorMode {
    Sql,
    Json,
}

#[derive(Clone)]
struct Token {
    text: String,
    kind: TokenKind,
}

#[derive(Clone, PartialEq, Eq)]
enum TokenKind {
    Keyword,
    String,
    Number,
    Comment,
    Punctuation,
    Plain,
}

const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "AND",
    "OR",
    "NOT",
    "IN",
    "IS",
    "NULL",
    "JOIN",
    "LEFT",
    "RIGHT",
    "INNER",
    "OUTER",
    "ON",
    "AS",
    "GROUP",
    "BY",
    "ORDER",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "DROP",
    "ALTER",
    "TABLE",
    "INDEX",
    "EXPLAIN",
    "ANALYZE",
    "WITH",
    "DISTINCT",
    "COUNT",
    "SUM",
    "AVG",
    "MAX",
    "MIN",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "EXISTS",
    "LIKE",
    "ILIKE",
    "BETWEEN",
    "UNION",
    "ALL",
    "RETURNING",
    "PRAGMA",
];

fn is_sql_keyword(word: &str) -> bool {
    SQL_KEYWORDS
        .iter()
        .any(|&kw| kw.eq_ignore_ascii_case(word.trim()))
}

fn tokenize_sql(input: &str) -> Vec<Token> {
    let mut tokens = vec![];
    let mut chars = input.chars().peekable();
    let mut current = String::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            '\'' => {
                if !current.is_empty() {
                    tokens.push(classify(std::mem::take(&mut current)));
                }
                let mut s = String::from('\'');
                chars.next();
                while let Some(&c) = chars.peek() {
                    s.push(c);
                    chars.next();
                    if c == '\'' {
                        break;
                    }
                }
                tokens.push(Token {
                    text: s,
                    kind: TokenKind::String,
                });
            }
            '-' => {
                chars.next();
                if chars.peek() == Some(&'-') {
                    if !current.is_empty() {
                        tokens.push(classify(std::mem::take(&mut current)));
                    }
                    let mut comment = String::from("--");
                    chars.next();
                    while let Some(&c) = chars.peek() {
                        if c == '\n' {
                            break;
                        }
                        comment.push(c);
                        chars.next();
                    }
                    tokens.push(Token {
                        text: comment,
                        kind: TokenKind::Comment,
                    });
                } else {
                    current.push('-');
                }
            }
            ' ' | '\n' | '\t' | ',' | '(' | ')' | ';' => {
                if !current.is_empty() {
                    tokens.push(classify(std::mem::take(&mut current)));
                }
                tokens.push(Token {
                    text: ch.to_string(),
                    kind: TokenKind::Punctuation,
                });
                chars.next();
            }
            _ => {
                current.push(ch);
                chars.next();
            }
        }
    }
    if !current.is_empty() {
        tokens.push(classify(current));
    }
    tokens
}

fn classify(text: String) -> Token {
    let kind = if is_sql_keyword(&text) {
        TokenKind::Keyword
    } else if !text.is_empty() && text.chars().all(|c| c.is_ascii_digit() || c == '.') {
        TokenKind::Number
    } else {
        TokenKind::Plain
    };
    Token { text, kind }
}

pub struct CodeEditor {
    pub content: String,
    pub mode: EditorMode,
    #[allow(dead_code)]
    pub read_only: bool,
}

impl CodeEditor {
    pub fn new(mode: EditorMode) -> Self {
        Self {
            content: String::new(),
            mode,
            read_only: false,
        }
    }
}

impl Render for CodeEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = if self.mode == EditorMode::Sql {
            tokenize_sql(&self.content)
        } else {
            vec![Token {
                text: self.content.clone(),
                kind: TokenKind::Plain,
            }]
        };

        let theme = cx.theme();
        // keyword: blue, string: amber, number: purple, comment: muted, plain: foreground
        let kw_color: Hsla = gpui::rgb(0x79c0ff).into();
        let str_color: Hsla = gpui::rgb(0xa5d6ff).into();
        let num_color: Hsla = gpui::rgb(0xa371f7).into();
        let mono = theme.mono_font_family.clone();

        div()
            .font_family(mono)
            .text_sm()
            .flex()
            .flex_wrap()
            .children(tokens.into_iter().map(|t| {
                let color = match t.kind {
                    TokenKind::Keyword => kw_color,
                    TokenKind::String => str_color,
                    TokenKind::Number => num_color,
                    TokenKind::Comment => theme.muted_foreground,
                    _ => theme.foreground,
                };
                div().text_color(color).child(t.text)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_classified() {
        let tokens = tokenize_sql("SELECT id FROM users WHERE id = 1");
        let kws: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Keyword)
            .map(|t| t.text.as_str())
            .collect();
        assert!(kws.iter().any(|s| s.eq_ignore_ascii_case("SELECT")));
        assert!(kws.iter().any(|s| s.eq_ignore_ascii_case("FROM")));
        assert!(kws.iter().any(|s| s.eq_ignore_ascii_case("WHERE")));
    }

    #[test]
    fn string_literal_classified() {
        let tokens = tokenize_sql("WHERE plan = 'pro'");
        let strings: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::String)
            .collect();
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].text, "'pro'");
    }

    #[test]
    fn comment_classified() {
        let tokens = tokenize_sql("SELECT 1 -- get one");
        let comments: Vec<_> = tokens
            .iter()
            .filter(|t| t.kind == TokenKind::Comment)
            .collect();
        assert!(!comments.is_empty());
        assert!(comments[0].text.contains("get one"));
    }
}
