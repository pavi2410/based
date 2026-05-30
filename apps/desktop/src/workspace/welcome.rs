use gpui::{
    App, Context, FocusHandle, Focusable, FontWeight, IntoElement, MouseButton, Render,
    SharedString, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _, StyledExt,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    tooltip::Tooltip,
    v_flex,
};

use crate::app::shell::OpenSettingsMenu;
use crate::bindings::ToggleCommandPalette;
use crate::widgets::ui::kbd_for_action;
use crate::workspace::WorkspaceRef;
use crate::workspace::query_lane::create_loose_query_from_palette;

const WELCOME_COLUMN_W: f32 = 480.0;
const COMING_SOON: &str = "Coming soon";

pub struct WelcomePanel {
    focus_handle: FocusHandle,
    pub(crate) tab_label: SharedString,
}

impl WelcomePanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            tab_label: "Welcome".into(),
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for WelcomePanel {}

impl Focusable for WelcomePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for WelcomePanel {
    fn panel_name(&self) -> &'static str {
        "WelcomePanel"
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

impl Render for WelcomePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette_kbd = kbd_for_action(&ToggleCommandPalette, window);
        let settings_kbd = kbd_for_action(&OpenSettingsMenu, window);

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w(px(WELCOME_COLUMN_W))
                    .gap(px(28.0))
                    .child(welcome_header(cx))
                    .child(
                        v_flex()
                            .gap(px(8.0))
                            .child(section_header("GET STARTED", cx))
                            .child(welcome_row(
                                cx,
                                "welcome-new-query",
                                IconName::Plus,
                                "New Query",
                                None,
                                true,
                                |_, _, cx| create_loose_query_from_palette(cx),
                            ))
                            .child(welcome_row(
                                cx,
                                "welcome-open-project",
                                IconName::FolderOpen,
                                "Open Project",
                                None,
                                false,
                                |_, _, _| {},
                            ))
                            .child(welcome_row(
                                cx,
                                "welcome-new-connection",
                                IconName::Globe,
                                "New Connection",
                                None,
                                true,
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
                            .child(welcome_row(
                                cx,
                                "welcome-command-palette",
                                IconName::Search,
                                "Open Command Palette",
                                palette_kbd,
                                true,
                                |_, window, cx| {
                                    if let Some(ws) =
                                        cx.try_global::<WorkspaceRef>().map(|w| w.0.clone())
                                    {
                                        ws.update(cx, |ws, cx| {
                                            ws.toggle_command_palette(window, cx);
                                        });
                                    }
                                },
                            )),
                    )
                    .child(
                        v_flex()
                            .gap(px(8.0))
                            .child(section_header("CONFIGURE", cx))
                            .child(welcome_row(
                                cx,
                                "welcome-settings",
                                IconName::Settings,
                                "Open Settings",
                                settings_kbd,
                                true,
                                |_, _, cx| crate::app::shell::open_settings(cx),
                            ))
                            .child(welcome_row(
                                cx,
                                "welcome-keymaps",
                                IconName::Settings2,
                                "Customize Keymaps",
                                None,
                                false,
                                |_, _, _| {},
                            ))
                            .child(welcome_row(
                                cx,
                                "welcome-extensions",
                                IconName::BookOpen,
                                "Explore Extensions",
                                None,
                                false,
                                |_, _, _| {},
                            )),
                    ),
            )
    }
}

fn welcome_header(cx: &mut App) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    v_flex()
        .items_center()
        .gap(px(8.0))
        .pb(px(4.0))
        .child(div().text_2xl().font_bold().text_color(fg).child("Based"))
        .child(
            div()
                .text_lg()
                .font_bold()
                .text_color(fg)
                .child("Welcome to Based"),
        )
        .child(
            div()
                .text_sm()
                .italic()
                .text_color(muted)
                .child("Git-Friendly Database Client"),
        )
}

fn section_header(label: &str, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label.to_string())
}

fn welcome_row(
    cx: &mut App,
    id: &'static str,
    icon: IconName,
    label: &'static str,
    shortcut: Option<gpui_component::kbd::Kbd>,
    enabled: bool,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let fg = cx.theme().foreground;
    let muted = cx.theme().muted_foreground;
    let hover_bg = cx.theme().muted.opacity(if enabled { 0.35 } else { 0.0 });

    let mut row = h_flex()
        .id(id)
        .w_full()
        .h(px(32.0))
        .px(px(8.0))
        .rounded(px(6.0))
        .items_center()
        .gap(px(10.0))
        .child(
            Icon::new(icon)
                .text_color(if enabled { muted } else { muted.opacity(0.5) })
                .with_size(gpui_component::Size::Small),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(if enabled { fg } else { muted.opacity(0.55) })
                .child(label),
        );

    if let Some(kbd) = shortcut {
        row = row.child(kbd);
    }

    if enabled {
        row = row
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_mouse_down(MouseButton::Left, on_click);
    } else {
        row = row.tooltip(move |window, app| {
            Tooltip::element(move |_w, tip_cx| {
                div()
                    .text_sm()
                    .text_color(tip_cx.theme().foreground)
                    .child(COMING_SOON)
            })
            .build(window, app)
        });
    }

    row
}
