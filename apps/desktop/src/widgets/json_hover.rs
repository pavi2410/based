//! Syntax-highlighted pretty JSON hover card for JSON column cells.

use std::ops::Range;

use gpui_kit::component::{ActiveTheme, hover_card::HoverCard};
use gpui_kit::{
    Anchor, AnyElement, ElementId, HighlightStyle, IntoElement, StyledText, div, prelude::*, px,
};

use crate::app::prefs;

/// Max chars of pretty JSON shown in the hover card.
const MAX_CHARS: usize = 4_000;

#[derive(Clone, Copy)]
enum Tok {
    Key,
    Str,
    Num,
    Lit,
}

/// Pretty-print `raw`; `None` when it isn't valid JSON.
fn pretty(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let mut out = serde_json::to_string_pretty(&value).ok()?;
    if let Some((idx, _)) = out.char_indices().nth(MAX_CHARS) {
        out.truncate(idx);
        out.push('…');
    }
    Some(out)
}

/// Byte ranges of highlightable tokens in pretty-printed JSON.
fn tokens(s: &str) -> Vec<(Range<usize>, Tok)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i = (i + 1).min(b.len());
                let is_key = b[i..].iter().find(|c| !c.is_ascii_whitespace()) == Some(&b':');
                out.push((start..i, if is_key { Tok::Key } else { Tok::Str }));
            }
            b'-' | b'0'..=b'9' => {
                let start = i;
                while i < b.len() && matches!(b[i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                {
                    i += 1;
                }
                out.push((start..i, Tok::Num));
            }
            b't' | b'f' | b'n' => {
                let start = i;
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                out.push((start..i, Tok::Lit));
            }
            _ => i += 1,
        }
    }
    out
}

/// Wrap a JSON cell in a hover card; returns the cell unchanged if `raw` isn't valid JSON.
pub fn wrap(id: impl Into<ElementId>, cell: impl IntoElement + 'static, raw: &str) -> AnyElement {
    let Some(text) = pretty(raw) else {
        return cell.into_any_element();
    };
    let toks = tokens(&text);
    HoverCard::new(id)
        .anchor(Anchor::TopLeft)
        .trigger(cell)
        .content(move |_, _, cx| {
            let t = cx.theme();
            let color = |tok| match tok {
                Tok::Key => t.blue_light,
                Tok::Str => t.green_light,
                Tok::Num => t.yellow_light,
                Tok::Lit => t.magenta_light,
            };
            let highlights = toks
                .iter()
                .map(|(r, tok)| (r.clone(), HighlightStyle::color(color(*tok))));
            div()
                .max_w(px(520.0))
                .max_h(px(360.0))
                .overflow_hidden()
                .text_xs()
                .font_family(prefs::code_font_family(cx))
                .text_color(t.foreground)
                .child(StyledText::new(text.clone()).with_highlights(highlights))
        })
        .into_any_element()
}
