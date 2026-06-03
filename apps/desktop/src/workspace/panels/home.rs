//! Chrome-style Home tab: logo + quick actions when no other editors are open.

use gpui::{
    App, Context, FocusHandle, Focusable, FontWeight, IntoElement, MouseButton, ParentElement,
    Render, SharedString, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    v_flex,
};

use crate::app::shell::OpenSettingsMenu;
use crate::bindings::{NewQuery, ToggleCommandPalette};
use crate::project::prompt_open_project_in_window;
use crate::widgets::kbd_for_action;
use crate::workspace::WorkspaceRef;
use crate::workspace::query_lane::create_loose_query_from_palette;

const HOME_COLUMN_W: f32 = 420.0;

pub struct HomePanel {
    focus_handle: FocusHandle,
    pub(crate) tab_label: SharedString,
}

impl HomePanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            tab_label: "Home".into(),
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for HomePanel {}

impl Focusable for HomePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for HomePanel {
    fn panel_name(&self) -> &'static str {
        "HomePanel"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    crate::based_panel_tab_chrome!();
}

impl Render for HomePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette_kbd = kbd_for_action(&ToggleCommandPalette, window);
        let new_query_kbd = kbd_for_action(&NewQuery, window);
        let settings_kbd = kbd_for_action(&OpenSettingsMenu, window);

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w(px(HOME_COLUMN_W))
                    .gap(px(20.0))
                    .child(home_logo(cx))
                    .child(
                        v_flex()
                            .gap(px(4.0))
                            .child(home_row(
                                cx,
                                "home-new-query",
                                IconName::Plus,
                                "New Query",
                                new_query_kbd,
                                |_, _, cx| create_loose_query_from_palette(cx),
                            ))
                            .child(home_row(
                                cx,
                                "home-command-palette",
                                IconName::Search,
                                "Show Command Palette",
                                palette_kbd,
                                |_, window, cx| {
                                    if let Some(ws) =
                                        cx.try_global::<WorkspaceRef>().map(|w| w.0.clone())
                                    {
                                        ws.update(cx, |ws, cx| {
                                            ws.toggle_command_palette(window, cx);
                                        });
                                    }
                                },
                            ))
                            .child(home_row(
                                cx,
                                "home-open-project",
                                IconName::FolderOpen,
                                "Open Project",
                                None,
                                |_, _, cx| prompt_open_project_in_window(cx),
                            ))
                            .child(home_row(
                                cx,
                                "home-new-connection",
                                IconName::Globe,
                                "New Connection",
                                None,
                                |_, window, cx| {
                                    if let Some(ws) =
                                        cx.try_global::<WorkspaceRef>().map(|w| w.0.clone())
                                    {
                                        ws.update(cx, |ws, cx| {
                                            ws.open_postgres_wizard_tab(window, cx);
                                        });
                                    }
                                },
                            ))
                            .child(home_row(
                                cx,
                                "home-settings",
                                IconName::Settings,
                                "Open Settings",
                                settings_kbd,
                                |_, _, cx| crate::app::shell::open_settings(cx),
                            )),
                    ),
            )
    }
}

fn home_logo(cx: &App) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    v_flex()
        .items_center()
        .gap(px(6.0))
        .pb(px(8.0))
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::BOLD)
                .text_color(fg.opacity(0.85))
                .child("Based"),
        )
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child("Git-friendly database client"),
        )
}

fn home_row(
    cx: &App,
    id: &'static str,
    icon: IconName,
    label: &'static str,
    shortcut: Option<gpui_component::kbd::Kbd>,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let hover_bg = cx.theme().muted.opacity(0.35);

    let mut row = h_flex()
        .id(id)
        .w_full()
        .h(px(32.0))
        .px(px(8.0))
        .rounded(px(6.0))
        .items_center()
        .gap(px(10.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, on_click)
        .child(
            Icon::new(icon)
                .text_color(muted)
                .with_size(crate::app::prefs::ui_component_size(cx)),
        )
        .child(div().flex_1().text_sm().text_color(fg).child(label));

    if let Some(kbd) = shortcut {
        row = row.child(kbd);
    }

    row
}
