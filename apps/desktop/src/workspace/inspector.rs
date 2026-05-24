use gpui::{Context, Entity, FontWeight, IntoElement, div, prelude::*};
use gpui_component::{ActiveTheme, StyledExt, h_flex, v_flex};

use crate::connection::{ConnectionEntry, ConnectionState};
use crate::widgets::ui::{engine_chip, engine_name, inspector_description_section, metadata_pill};

use super::Workspace;
use super::notify;

pub(crate) fn render_inspector(
    selected: Option<Entity<ConnectionEntry>>,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let border = cx.theme().border;
    let muted = cx.theme().muted_foreground;

    let content = if let Some(ent) = selected {
        let entry = ent.read(cx);
        let engine = entry.config.engine();
        let label = entry.config.label().to_string();
        let state = entry.state.label().to_string();
        let summary = match &entry.state {
            ConnectionState::Failed { reason, .. } => notify::error_one_liner(reason).to_string(),
            ConnectionState::Connected(_) => "Ready for browsing and queries".to_string(),
            ConnectionState::Connecting { .. } => "Opening connection".to_string(),
            ConnectionState::Disconnected => "Click to connect".to_string(),
        };

        v_flex()
            .gap_3()
            .p_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .truncate()
                            .child(label),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(format!("{} connection", engine_name(engine))),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(engine_chip(engine, cx))
                    .child(metadata_pill("state", state, cx)),
            )
            .child(inspector_description_section(
                "Activity",
                [
                    ("Recent", "Schema refresh"),
                    ("Saved", "0 queries"),
                    ("Pinned", "No pinned objects"),
                ],
                cx,
            ))
            .child(inspector_note("Health", &summary, cx))
            .into_any_element()
    } else {
        v_flex()
            .gap_3()
            .p_3()
            .child(inspector_note(
                "Selection",
                "Choose a connection, table, cell, or query to see details here.",
                cx,
            ))
            .child(inspector_description_section(
                "Shortcuts",
                [
                    (
                        "Command",
                        if cfg!(target_os = "macos") {
                            "⌘K"
                        } else {
                            "Ctrl K"
                        },
                    ),
                    (
                        "Run query",
                        if cfg!(target_os = "macos") {
                            "⌘↵"
                        } else {
                            "Ctrl Enter"
                        },
                    ),
                    (
                        "Sidebar",
                        if cfg!(target_os = "macos") {
                            "⌘\\"
                        } else {
                            "Ctrl \\"
                        },
                    ),
                ],
                cx,
            ))
            .into_any_element()
    };

    v_flex()
        .w(gpui::px(286.0))
        .h_full()
        .flex_shrink_0()
        .border_l_1()
        .border_color(border)
        .bg(cx.theme().background)
        .child(
            h_flex()
                .h(gpui::px(38.0))
                .px_3()
                .items_center()
                .border_b_1()
                .border_color(border.opacity(0.86))
                .child(
                    div()
                        .text_xs()
                        .font_bold()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_color(muted)
                        .child("INSPECTOR"),
                ),
        )
        .child(content)
}

fn inspector_note(
    title: &'static str,
    body: &str,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .p_2()
        .rounded(gpui::px(crate::widgets::ui::PANEL_RADIUS))
        .border_1()
        .border_color(cx.theme().border.opacity(0.85))
        .bg(cx.theme().muted.opacity(0.28))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(body.to_string()),
        )
}
