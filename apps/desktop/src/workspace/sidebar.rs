use gpui::{App, IntoElement, RenderOnce, div, prelude::*};
use gpui_component::{ActiveTheme, StyledExt, h_flex, v_flex};

use crate::connection::{ConnectionEntry, ConnectionState, EngineKind};

/// A plain div-based sidebar showing the list of connections with state glyphs.
/// Owns its data so it can satisfy `RenderOnce`'s `'static` requirement.
#[derive(IntoElement)]
pub struct ConnectionSidebar {
    connections: Vec<ConnectionEntry>,
}

impl ConnectionSidebar {
    pub fn new(connections: Vec<ConnectionEntry>) -> Self {
        Self { connections }
    }
}

impl RenderOnce for ConnectionSidebar {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .w(gpui::px(220.0))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .bg(cx.theme().sidebar)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .font_bold()
                    .text_color(cx.theme().muted_foreground)
                    .child("CONNECTIONS"),
            )
            .children(
                self.connections
                    .into_iter()
                    .enumerate()
                    .map(|(idx, entry)| {
                        let state_color = state_dot_color(&entry.state);
                        let engine_label = entry.config.engine().short_label();
                        let conn_label = entry.config.label().to_string();
                        let state_label = entry.state.label();
                        let badge_color = engine_badge_color(entry.config.engine());

                        h_flex()
                            .id(("conn-row", idx))
                            .px_3()
                            .py_2()
                            .gap_2()
                            .items_center()
                            .cursor_pointer()
                            .rounded_md()
                            .mx_1()
                            .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.5, 0.08)))
                            .on_click(move |_, _window, _cx| {
                                eprintln!("connection clicked: {} ({})", conn_label, state_label);
                            })
                            .child(
                                div()
                                    .w_2()
                                    .h_2()
                                    .rounded_full()
                                    .flex_shrink_0()
                                    .bg(state_color),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .px_1()
                                    .rounded_sm()
                                    .bg(badge_color)
                                    .text_color(cx.theme().foreground)
                                    .child(engine_label),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(cx.theme().sidebar_foreground)
                                    .truncate()
                                    .child(entry.config.label().to_string()),
                            )
                    }),
            )
    }
}

fn state_dot_color(state: &ConnectionState) -> gpui::Hsla {
    match state {
        ConnectionState::Disconnected => gpui::hsla(0.0, 0.0, 0.5, 1.0),
        ConnectionState::Connecting { .. } => gpui::hsla(0.13, 0.95, 0.55, 1.0),
        ConnectionState::Connected(_) => gpui::hsla(0.35, 0.75, 0.45, 1.0),
        ConnectionState::Failed { .. } => gpui::hsla(0.0, 0.75, 0.5, 1.0),
    }
}

fn engine_badge_color(engine: EngineKind) -> gpui::Hsla {
    match engine {
        EngineKind::Postgres => gpui::hsla(0.58, 0.6, 0.35, 0.3),
        EngineKind::MongoDB => gpui::hsla(0.28, 0.6, 0.35, 0.3),
        EngineKind::SQLite => gpui::hsla(0.08, 0.5, 0.4, 0.3),
    }
}
