use gpui::{App, Context, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    v_flex,
};

use crate::connection::EngineKind;
use crate::widgets::ui::{engine_chip, metadata_pill, panel_header};

pub struct ConnectionDashboardPanel {
    focus_handle: FocusHandle,
    label: String,
    engine: EngineKind,
}

impl ConnectionDashboardPanel {
    pub fn new(
        label: String,
        engine: EngineKind,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label,
            engine,
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for ConnectionDashboardPanel {}

impl Focusable for ConnectionDashboardPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ConnectionDashboardPanel {
    fn panel_name(&self) -> &'static str {
        "ConnectionDashboard"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.label.clone()
    }
}

impl Render for ConnectionDashboardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(panel_header(
                self.label.clone(),
                "Connection dashboard",
                cx,
            ))
            .child(
                v_flex()
                    .p_4()
                    .gap_4()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(engine_chip(self.engine, cx))
                            .child(metadata_pill("state", "connected", cx))
                            .child(metadata_pill("scope", "local workspace", cx)),
                    )
                    .child(dashboard_card(
                        "Start",
                        "Use the Objects pane to open tables, views, or collections. Use the query tab for ad-hoc work.",
                        cx,
                    ))
                    .child(dashboard_card(
                        "Workflow",
                        "Objects stay in the sidebar; work opens as tabs in the center.",
                        cx,
                    )),
            )
    }
}

pub struct ObjectInfoPanel {
    focus_handle: FocusHandle,
    name: String,
    kind: String,
}

impl ObjectInfoPanel {
    pub fn new(
        name: String,
        kind: impl Into<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            name,
            kind: kind.into(),
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for ObjectInfoPanel {}

impl Focusable for ObjectInfoPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ObjectInfoPanel {
    fn panel_name(&self) -> &'static str {
        "ObjectInfo"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.name.clone()
    }
}

impl Render for ObjectInfoPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(panel_header(
                self.name.clone(),
                format!("{} metadata", self.kind),
                cx,
            ))
            .child(
                v_flex()
                    .p_4()
                    .gap_3()
                    .child(metadata_pill("object", self.name.clone(), cx))
                    .child(metadata_pill("kind", self.kind.clone(), cx))
                    .child(dashboard_card(
                        "Inspector",
                        "Detailed schema, DDL, and dependency views will live here as the object model matures.",
                        cx,
                    )),
            )
    }
}

fn dashboard_card(title: &'static str, body: &'static str, cx: &mut App) -> impl IntoElement {
    v_flex()
        .gap_1()
        .p_3()
        .max_w(gpui::px(560.0))
        .rounded(gpui::px(8.0))
        .border_1()
        .border_color(cx.theme().border.opacity(0.82))
        .bg(cx.theme().muted.opacity(0.22))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(body),
        )
}
