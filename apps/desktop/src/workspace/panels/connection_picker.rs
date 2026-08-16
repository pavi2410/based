//! Engine picker for New Connection — lists registered engines, then opens a wizard.

use gpui::{
    App, Context, EntityId, FocusHandle, Focusable, FontWeight, IntoElement, MouseButton,
    ParentElement, Render, SharedString, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    v_flex,
};

use crate::based_panel_dropdown;
use crate::based_panel_tab_chrome;
use crate::connection::{EngineKind, EngineRegistry};
use crate::widgets::engine_icon;
use crate::workspace::WorkspaceRef;

const PICKER_COLUMN_W: f32 = 420.0;

pub struct ConnectionPickerPanel {
    focus_handle: FocusHandle,
    pub(crate) tab_label: SharedString,
}

impl ConnectionPickerPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            tab_label: "New connection".into(),
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for ConnectionPickerPanel {}

impl Focusable for ConnectionPickerPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ConnectionPickerPanel {
    fn panel_name(&self) -> &'static str {
        "ConnectionPicker"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        based_panel_dropdown!(menu, self, cx)
    }

    based_panel_tab_chrome!();
}

impl Render for ConnectionPickerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let picker_id = cx.entity().entity_id();
        let engines: Vec<(EngineKind, SharedString, SharedString)> = cx
            .global::<EngineRegistry>()
            .all()
            .iter()
            .map(|d| {
                (
                    d.kind(),
                    SharedString::from(d.display_name().to_string()),
                    SharedString::from(d.connect_hint().to_string()),
                )
            })
            .collect();

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w(px(PICKER_COLUMN_W))
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Choose a database engine"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Opens a connection form for the selected engine."),
                    )
                    .child(
                        v_flex().gap(px(6.0)).children(
                            engines.into_iter().map(|(kind, name, hint)| {
                                engine_row(cx, picker_id, kind, name, hint)
                            }),
                        ),
                    ),
            )
    }
}

fn engine_row(
    cx: &App,
    picker_id: EntityId,
    kind: EngineKind,
    name: SharedString,
    hint: SharedString,
) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let hover_bg = cx.theme().muted.opacity(0.35);
    let border = cx.theme().border;

    h_flex()
        .id(SharedString::from(format!(
            "picker-engine-{}",
            kind.as_str()
        )))
        .w_full()
        .px(px(12.0))
        .py(px(10.0))
        .gap(px(12.0))
        .items_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(border)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                ws.update(cx, |ws, cx| {
                    ws.open_wizard_replacing_picker(kind, picker_id, window, cx);
                });
            }
        })
        .child(engine_icon(kind))
        .child(
            v_flex()
                .gap(px(2.0))
                .flex_1()
                .child(div().text_sm().text_color(fg).child(name))
                .child(div().text_xs().text_color(muted).child(hint)),
        )
}
