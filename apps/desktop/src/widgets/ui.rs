//! Shared graphite-native UI primitives for the workspace.

use gpui::{img, prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    description_list::{DescriptionItem, DescriptionList},
    h_flex,
    kbd::Kbd,
    v_flex,
};

use crate::app::prefs::{self, panel_header_h, sidebar_row_gap, sidebar_row_py};
use crate::bindings::{DismissCommandPalette, ToggleCommandPalette, ToggleSidebarRail};
use crate::connection::EngineKind;

/// Unbound / literal keys — `Kbd` formats symbols vs labels per platform.
pub fn kbd(stroke: &str) -> Kbd {
    Kbd::new(Keystroke::parse(stroke).expect("valid keystroke"))
}

/// Bound global action — uses the stroke GPUI registered (`cmd-*` vs `ctrl-*`).
pub fn kbd_for_action(action: &dyn Action, window: &Window) -> Option<Kbd> {
    Kbd::binding_for_action(action, None, window)
}

/// Run query (secondary Enter); not a global `KeyBinding` yet.
pub fn shortcut_run_kbd() -> Kbd {
    if cfg!(target_os = "macos") {
        kbd("cmd-enter")
    } else {
        kbd("ctrl-enter")
    }
}

/// Run shortcut on the primary Run button — default `Kbd` fill (not `.outline()`), tuned for inverted button colors.
pub fn shortcut_run_kbd_in_primary_button(cx: &App) -> Kbd {
    let fg = cx.theme().button_primary_foreground;
    shortcut_run_kbd()
        .text_color(fg.opacity(0.92))
        .bg(fg.opacity(0.18))
}

/// Inspector empty-state shortcut list with styled `Kbd` chips.
pub fn inspector_shortcuts_section(window: &Window, cx: &mut App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let palette = kbd_for_action(&ToggleCommandPalette, window).unwrap_or_else(|| kbd("cmd-k"));
    let sidebar = kbd_for_action(&ToggleSidebarRail, window).unwrap_or_else(|| kbd("cmd-\\"));
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_bold()
                .font_family(prefs::ui_font_family(cx))
                .font_weight(prefs::ui_font_weight(cx))
                .text_color(muted)
                .child("Shortcuts"),
        )
        .child(
            v_flex()
                .gap_2()
                .child(shortcut_row_styled("Command", palette, muted))
                .child(shortcut_row_styled("Run query", shortcut_run_kbd(), muted))
                .child(shortcut_row_styled("Sidebar", sidebar, muted)),
        )
}

fn shortcut_row_styled(label: &'static str, kbd_el: Kbd, muted: Hsla) -> impl IntoElement {
    h_flex()
        .w_full()
        .justify_between()
        .items_center()
        .child(div().text_xs().text_color(muted).child(label))
        .child(kbd_el)
}

/// Command palette footer key hints.
pub fn palette_footer_hints(window: &Window, cx: &mut App) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let dismiss = kbd_for_action(&DismissCommandPalette, window).unwrap_or_else(|| kbd("escape"));
    h_flex()
        .gap_2()
        .items_center()
        .text_xs()
        .text_color(muted)
        .child(kbd("up"))
        .child(kbd("down"))
        .child("navigate")
        .child("·")
        .child(kbd("enter"))
        .child("open")
        .child("·")
        .child(shortcut_run_kbd())
        .child("query")
        .child("·")
        .child(dismiss)
        .child("dismiss")
}

/// Boxy panel corner radius (Linear / Vercel–style).
pub const PANEL_RADIUS: f32 = 4.0;
/// Horizontal inset for sidebar list rows and section headers.
pub const SIDEBAR_INSET: f32 = 8.0;
/// Fixed lead column so engine icons align across connected / disconnected rows.
pub const CONNECTION_CHEVRON_SLOT_W: f32 = 18.0;

pub fn panel_header_height(cx: &App) -> f32 {
    panel_header_h(prefs::ui_size_token(cx))
}

pub fn sidebar_row_padding_y(cx: &App) -> f32 {
    sidebar_row_py(prefs::ui_size_token(cx))
}

pub fn sidebar_row_inner_gap(cx: &App) -> f32 {
    sidebar_row_gap(prefs::ui_size_token(cx))
}

/// Browser tree: left edge of the engine-icon column on connection rows.
pub fn browser_tree_engine_col(cx: &App) -> f32 {
    SIDEBAR_INSET + CONNECTION_CHEVRON_SLOT_W + sidebar_row_inner_gap(cx)
}
/// Browser tree: section headers (Views / Tables).
pub fn browser_tree_section_pl(cx: &App) -> f32 {
    browser_tree_engine_col(cx)
}
/// Browser tree: schema object rows (one step under sections).
pub fn browser_tree_object_pl(cx: &App) -> f32 {
    browser_tree_engine_col(cx) + 10.0
}

/// Schema object row kind icon size.
pub const SCHEMA_ROW_ICON_SIZE: f32 = 14.0;

pub fn engine_label(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "PG",
        EngineKind::MongoDB => "MDB",
        EngineKind::SQLite => "SQL",
    }
}

pub fn engine_name(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "Postgres",
        EngineKind::MongoDB => "MongoDB",
        EngineKind::SQLite => "SQLite",
    }
}

pub fn engine_color(engine: EngineKind) -> Hsla {
    match engine {
        EngineKind::Postgres => hsla(0.58, 0.78, 0.64, 1.0),
        EngineKind::MongoDB => hsla(0.38, 0.62, 0.54, 1.0),
        EngineKind::SQLite => hsla(0.10, 0.74, 0.59, 1.0),
    }
}

fn engine_icon_path(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Postgres => "icons/engines/postgresql.svg",
        EngineKind::MongoDB => "icons/engines/mongodb.svg",
        EngineKind::SQLite => "icons/engines/sqlite.svg",
    }
}

/// Devicon colored brand mark (rasterized via `img` so fills are not mono-tinted).
pub fn engine_icon(engine: EngineKind) -> impl IntoElement {
    img(engine_icon_path(engine)).size(px(16.0)).flex_shrink_0()
}

/// Compact engine label for sidebar rows (no border or fill).
pub fn engine_label_inline(engine: EngineKind, cx: &mut App) -> impl IntoElement {
    let color = engine_color(engine);
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .font_family(prefs::ui_font_family(cx))
        .font_weight(prefs::ui_font_weight(cx))
        .text_color(color)
        .child(engine_label(engine))
}

pub fn engine_chip(engine: EngineKind, cx: &mut App) -> impl IntoElement {
    let color = engine_color(engine);
    h_flex()
        .h(px(18.0))
        .px(px(6.0))
        .gap(px(4.0))
        .items_center()
        .rounded(px(4.0))
        .bg(color.opacity(0.13))
        .border_1()
        .border_color(color.opacity(0.18))
        .child(
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded_full()
                .bg(color.opacity(0.9)),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .font_family(prefs::ui_font_family(cx))
                .font_weight(prefs::ui_font_weight(cx))
                .text_color(color)
                .child(engine_label(engine)),
        )
}

/// Horizontal key-value list for narrow panels (e.g. the right inspector).
pub fn compact_description_list_horizontal(
    rows: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
) -> DescriptionList {
    let items: Vec<DescriptionItem> = rows
        .into_iter()
        .map(|(label, value)| {
            let label: SharedString = label.into();
            let value: SharedString = value.into();
            DescriptionItem::new(label).value(value)
        })
        .collect();
    // `DescriptionList` defaults to 3 columns per row, which character-wraps inside
    // the 320 px Inspector side pane. Force one item per row so labels and values
    // get the full column width.
    DescriptionList::horizontal()
        .small()
        .bordered(false)
        .columns(1)
        .label_width(px(88.0))
        .children(items)
}

/// Vertical key-value list for dashboard cards and schema stats.
pub fn compact_description_list_vertical(
    rows: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    bordered: bool,
) -> DescriptionList {
    let items: Vec<DescriptionItem> = rows
        .into_iter()
        .map(|(label, value)| {
            let label: SharedString = label.into();
            let value: SharedString = value.into();
            DescriptionItem::new(label).value(value)
        })
        .collect();
    DescriptionList::vertical()
        .small()
        .bordered(bordered)
        .children(items)
}

/// Inspector sidebar section: mono title + compact horizontal description list.
pub fn inspector_description_section(
    title: &'static str,
    rows: impl IntoIterator<Item = (&'static str, impl Into<SharedString>)>,
    cx: &mut App,
) -> impl IntoElement {
    let items: Vec<(SharedString, SharedString)> = rows
        .into_iter()
        .map(|(label, value)| (label.into(), value.into()))
        .collect();
    v_flex()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_bold()
                .font_family(prefs::ui_font_family(cx))
                .font_weight(prefs::ui_font_weight(cx))
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .child(compact_description_list_horizontal(items))
}

pub fn metadata_pill(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    h_flex()
        .h(px(20.0))
        .items_center()
        .gap(px(5.0))
        .px(px(7.0))
        .rounded(px(5.0))
        .border_1()
        .border_color(cx.theme().border.opacity(0.72))
        .bg(cx.theme().muted.opacity(0.34))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label.into()),
        )
        .child(
            div()
                .text_xs()
                .font_family(prefs::code_font_family(cx))
                .text_color(cx.theme().foreground.opacity(0.9))
                .child(value.into()),
        )
}

pub fn command_shell(window: &Window, cx: &mut App, placeholder: &'static str) -> impl IntoElement {
    let hint = kbd_for_action(&ToggleCommandPalette, window).unwrap_or_else(|| kbd("cmd-k"));
    h_flex()
        .id("global-command-shell")
        .h(px(28.0))
        .w(px(430.0))
        .max_w(px(520.0))
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(cx.theme().input)
        .bg(cx.theme().tab_active)
        .shadow_xs()
        .hover(|s| s.border_color(cx.theme().border))
        .child(
            Icon::new(IconName::Search)
                .text_color(cx.theme().muted_foreground)
                .with_size(prefs::ui_component_size(cx).smaller()),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(placeholder),
        )
        .child(hint)
}

/// Compact title strip for a boxed panel.
pub fn panel_shell_header(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    let border = cx.theme().border.opacity(0.85);
    h_flex()
        .h(px(panel_header_height(cx)))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .px(px(10.0))
        .border_b_1()
        .border_color(border)
        .bg(cx.theme().muted.opacity(0.22))
        .child(
            v_flex()
                .min_w_0()
                .gap(px(1.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .truncate()
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(subtitle.into()),
                ),
        )
}

pub fn panel_header(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &mut App,
) -> impl IntoElement {
    panel_shell_header(title, subtitle, cx)
}

/// In-panel context line when the dock tab already shows the title.
pub fn panel_context_header(subtitle: impl Into<SharedString>, cx: &mut App) -> impl IntoElement {
    let border = cx.theme().border.opacity(0.85);
    h_flex()
        .h(px(panel_header_height(cx)))
        .w_full()
        .flex_shrink_0()
        .items_center()
        .px(px(10.0))
        .border_b_1()
        .border_color(border)
        .bg(cx.theme().muted.opacity(0.18))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(subtitle.into()),
        )
}

/// Secondary toolbar row inside a panel shell.
pub fn toolbar_strip(
    cx: &mut App,
    children: impl IntoIterator<Item = AnyElement>,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .flex_shrink_0()
        .flex_wrap()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.72))
        .bg(cx.theme().muted.opacity(0.18))
        .children(children)
}

/// Bordered panel frame: optional header + flexible body.
pub fn panel_shell(
    cx: &mut App,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    body: impl IntoElement,
) -> impl IntoElement {
    let border = cx.theme().border.opacity(0.85);
    let title = title.into();
    let subtitle = subtitle.into();
    let header: AnyElement = if title.is_empty() {
        panel_context_header(subtitle, cx).into_any_element()
    } else {
        panel_shell_header(title, subtitle, cx).into_any_element()
    };
    v_flex()
        .size_full()
        .border_1()
        .border_color(border)
        .rounded(px(PANEL_RADIUS))
        .overflow_hidden()
        .bg(cx.theme().background)
        .child(header)
        .child(div().flex_1().min_h_0().child(body))
}

pub fn toolbar_button(id: &'static str, icon: IconName, tooltip: &'static str, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .with_size(prefs::ui_component_size(cx).smaller())
        .icon(icon)
        .tooltip(SharedString::from(tooltip))
}

pub fn empty_state(
    title: &'static str,
    body: &'static str,
    icon: IconName,
    cx: &mut App,
) -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(
            div()
                .w(px(42.0))
                .h(px(42.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(cx.theme().border.opacity(0.82))
                .bg(cx.theme().muted.opacity(0.4))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .text_color(cx.theme().muted_foreground)
                        .with_size(prefs::ui_component_size(cx)),
                ),
        )
        .child(
            v_flex()
                .items_center()
                .gap(px(3.0))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(body),
                ),
        )
}
