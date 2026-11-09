//! SQL in a finished reply becomes a card: read-only badge, Run, Insert in editor, Copy.

use std::mem::take;

use based_ai::tools::check;
use based_core::EngineKind;
use based_project::save_sql_query;
use gpui_kit::component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, App, ClipboardItem, ElementId, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement as _, Styled, div, px,
};

use super::style;
use crate::app::prefs;
use crate::connection::ConnectionId;
use crate::project::ProjectRoot;
use crate::workspace::notify;
use crate::workspace::panels::query_editor::save_target_for;
use crate::workspace::tabs::inbox::{self, request_workspace_flush};
use crate::workspace::tabs::spec::{QueryEditorInit, TabSpec};

/// A piece of a reply: prose, or one fenced SQL block.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Segment {
    Text(String),
    Sql(String),
}

/// Splits `text` at fenced blocks tagged `sql`. Other fences stay inside the prose.
pub(super) fn split_reply(text: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut prose = String::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let opens = line
            .trim_start()
            .strip_prefix("```")
            .is_some_and(|tag| tag.trim().eq_ignore_ascii_case("sql"));
        if !opens {
            prose.push_str(line);
            prose.push('\n');
            continue;
        }
        let mut body = String::new();
        let mut closed = false;
        for inner in lines.by_ref() {
            if inner.trim_start().starts_with("```") {
                closed = true;
                break;
            }
            body.push_str(inner);
            body.push('\n');
        }
        if !closed || body.trim().is_empty() {
            // Unfinished or empty: leave it to the markdown renderer.
            prose.push_str(line);
            prose.push('\n');
            prose.push_str(&body);
            continue;
        }
        if !prose.trim().is_empty() {
            out.push(Segment::Text(take(&mut prose)));
        } else {
            prose.clear();
        }
        out.push(Segment::Sql(body.trim_end().to_owned()));
    }
    if !prose.trim().is_empty() {
        out.push(Segment::Text(prose));
    }
    out
}

/// Cards run SQL, so only the SQL engines get them.
pub(super) fn supports_cards(engine: EngineKind) -> bool {
    matches!(engine, EngineKind::Postgres | EngineKind::SQLite)
}

fn open_in_editor(conn: &ConnectionId, sql: String, run: bool, cx: &mut App) {
    if run {
        let init = QueryEditorInit::Sql {
            sql: Some(sql),
            auto_run: true,
        };
        inbox::open_tab(
            TabSpec::QueryEditor {
                conn_id: conn.clone(),
                init,
            },
            cx,
        );
    } else {
        inbox::inject_sql(conn.clone(), sql, cx);
    }
    request_workspace_flush(cx);
}

/// Name for a saved card query: its first `--` comment, else the opening words.
fn query_name(sql: &str) -> String {
    let comment = sql
        .lines()
        .next()
        .and_then(|l| l.trim().strip_prefix("--"))
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty());
    comment.unwrap_or_else(|| {
        let words: Vec<&str> = sql.split_whitespace().take(6).collect();
        format!("Assistant: {}", words.join(" "))
    })
}

/// Writes the card's SQL into the project's queries folder.
fn save_to_project(conn: &ConnectionId, engine: EngineKind, sql: &str, cx: &mut App) {
    let Some(root) = cx.try_global::<ProjectRoot>().map(|r| r.0.clone()) else {
        notify::push_info(cx, "Open a project to save queries.");
        return;
    };
    let target = save_target_for(conn, engine, cx);
    let name = query_name(sql);
    match save_sql_query(&root, &name, sql, &target, None) {
        Ok(_) => notify::push_info(cx, format!("Saved {name}")),
        Err(e) => notify::push_error(cx, "Could not save query", e.to_string()),
    }
}

pub(super) fn sql_card(
    id: impl Into<ElementId>,
    sql: &str,
    engine: EngineKind,
    conn: &ConnectionId,
    cx: &App,
) -> AnyElement {
    let id: ElementId = id.into();
    let theme = cx.theme();
    let read_only = check(engine, sql).is_ok();
    let (badge, tone) = if read_only {
        ("read-only", theme.success)
    } else {
        ("changes data", theme.warning)
    };
    let (border, muted, panel) = (
        theme.border,
        theme.muted_foreground,
        theme.muted.opacity(0.2),
    );
    let key = |suffix: &str| -> SharedString { format!("{id}-{suffix}").into() };
    let button = |suffix: &str| Button::new(key(suffix)).small();
    let (run_sql, insert_sql, copy_sql) = (sql.to_owned(), sql.to_owned(), sql.to_owned());
    let (run_conn, insert_conn, save_conn) = (conn.clone(), conn.clone(), conn.clone());
    let save_sql = sql.to_owned();
    let mut footer = h_flex().gap_1().px_2().py_1p5().items_center().flex_wrap();
    if read_only {
        footer = footer.child(
            button("run")
                .primary()
                .label("Run")
                .on_click(move |_, _, cx| open_in_editor(&run_conn, run_sql.clone(), true, cx)),
        );
    }
    footer = footer
        .child(
            button("insert")
                .label("Insert in editor")
                .on_click(move |_, _, cx| {
                    open_in_editor(&insert_conn, insert_sql.clone(), false, cx);
                }),
        )
        .child(
            button("save")
                .ghost()
                .label("Save as query")
                .on_click(move |_, _, cx| save_to_project(&save_conn, engine, &save_sql, cx)),
        )
        .child(div().flex_1())
        .child(
            button("copy")
                .ghost()
                .label("Copy")
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_sql.clone()));
                }),
        );
    v_flex()
        .my_1()
        .rounded_lg()
        .border_1()
        .border_color(border)
        .overflow_hidden()
        .child(
            h_flex()
                .px_2()
                .py_1()
                .gap_2()
                .items_center()
                .border_b_1()
                .border_color(border)
                .text_size(style::small(cx))
                .text_color(muted)
                .child(div().text_color(tone).child(badge))
                .child(div().child(engine.short_label())),
        )
        .child(
            div()
                .id(key("code"))
                .px_3()
                .py_2()
                .bg(panel)
                .max_h(px(260.0))
                .overflow_scroll()
                .font_family(prefs::code_font_family(cx))
                .text_size(style::code(cx))
                .child(sql.to_owned()),
        )
        .child(footer)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_prose_and_sql() {
        let reply = "Here you go:\n```sql\nSELECT 1\nFROM t;\n```\nThat counts rows.\n";
        assert_eq!(
            split_reply(reply),
            vec![
                Segment::Text("Here you go:\n".into()),
                Segment::Sql("SELECT 1\nFROM t;".into()),
                Segment::Text("That counts rows.\n".into()),
            ]
        );
    }

    #[test]
    fn other_fences_and_unfinished_sql_stay_prose() {
        assert_eq!(
            split_reply("```json\n{}\n```\n"),
            vec![Segment::Text("```json\n{}\n```\n".into())]
        );
        let open = split_reply("```sql\nSELECT");
        assert!(matches!(open.as_slice(), [Segment::Text(t)] if t.contains("SELECT")));
    }

    #[test]
    fn names_come_from_the_comment_or_the_opening_words() {
        assert_eq!(query_name("-- Top customers\nSELECT 1"), "Top customers");
        assert_eq!(
            query_name("SELECT a, b FROM t WHERE x > 1 ORDER BY a"),
            "Assistant: SELECT a, b FROM t WHERE"
        );
    }

    #[test]
    fn two_blocks_in_one_reply() {
        let r = "```sql\nA\n```\n```SQL\nB\n```";
        assert_eq!(
            split_reply(r),
            vec![Segment::Sql("A".into()), Segment::Sql("B".into())]
        );
    }
}
