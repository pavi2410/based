use gpui::{AnyElement, Context, Entity, FontWeight, IntoElement, Window, div, prelude::*};
use gpui_component::{ActiveTheme, h_flex, v_flex};

use crate::connection::{ConnectionEntry, ConnectionState};
use crate::widgets::{
    engine_chip, engine_name, inspector_description_section, inspector_shortcuts_section,
    metadata_pill,
};

use crate::workspace::Workspace;
use crate::workspace::notify;

/// Body of the Inspector side pane. The 320 px column chrome (border + header)
/// comes from [`crate::workspace::chrome::side_pane::render_side_pane`].
pub(crate) fn render_inspector_body(
    selected: Option<Entity<ConnectionEntry>>,
    window: &Window,
    cx: &mut Context<Workspace>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;

    if let Some(ent) = selected {
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
            .child(inspector_shortcuts_section(window, cx))
            .into_any_element()
    }
}

fn inspector_note(
    title: &'static str,
    body: &str,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .p_2()
        .rounded(gpui::px(crate::widgets::PANEL_RADIUS))
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
