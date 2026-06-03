//! Keyboard shortcut helpers — unbound literal keys and bound action lookups.

use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, StyledExt, h_flex, kbd::Kbd, v_flex};

use crate::app::prefs;
use crate::bindings::{DismissCommandPalette, ToggleCommandPalette, ToggleSidebarRail};

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

fn palette_hint_kbd(k: Kbd, cx: &App) -> Kbd {
    k.outline().text_color(cx.theme().foreground)
}

/// Command palette footer key hints.
pub fn palette_footer_hints(window: &Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let dismiss = kbd_for_action(&DismissCommandPalette, window).unwrap_or_else(|| kbd("escape"));
    h_flex()
        .flex_shrink_0()
        .w_full()
        .px_3()
        .py_2()
        .gap_2()
        .items_center()
        .border_t_1()
        .border_color(theme.border)
        .bg(theme.muted.opacity(0.12))
        .text_xs()
        .text_color(muted)
        .child(palette_hint_kbd(kbd("up"), cx))
        .child(palette_hint_kbd(kbd("down"), cx))
        .child("navigate")
        .child("·")
        .child(palette_hint_kbd(kbd("enter"), cx))
        .child("open")
        .child("·")
        .child(palette_hint_kbd(shortcut_run_kbd(), cx))
        .child("query")
        .child("·")
        .child(palette_hint_kbd(dismiss, cx))
        .child("dismiss")
}
