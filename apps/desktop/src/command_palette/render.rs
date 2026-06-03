use gpui::{Context, IntoElement, MouseButton, Render, SharedString, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme, Icon, IconName, input::Input, scroll::Scrollbar, v_flex};

use crate::widgets::list_row::palette_result_row;
use crate::widgets::palette_footer_hints;

use super::CommandPalette;
use super::actions::{PaletteConfirm, PaletteSelectDown, PaletteSelectUp};
use super::format;

const PALETTE_LIST_H: f32 = 360.0;

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element();
        }

        if let Some(ix) = self.take_pending_scroll() {
            self.results_scroll.scroll_to_item(ix);
        }

        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let fg = theme.foreground;

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dismiss(cx);
                }),
            )
            .child(
                v_flex()
                    .absolute()
                    .top(px(120.0))
                    .left_1_2()
                    .ml(px(-280.0))
                    .w(px(560.0))
                    .max_h(px(480.0))
                    .overflow_hidden()
                    .track_focus(&self.focus_handle)
                    .key_context("CommandPalette")
                    .on_action(cx.listener(|this, _: &PaletteSelectUp, _, cx| {
                        this.select_prev(cx);
                    }))
                    .on_action(cx.listener(|this, _: &PaletteSelectDown, _, cx| {
                        this.select_next(cx);
                    }))
                    .on_action(cx.listener(|this, action: &PaletteConfirm, _, cx| {
                        this.open_selected(action.secondary, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        div()
                            .flex_shrink_0()
                            .p_2()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                Input::new(&self.search_input)
                                    .appearance(false)
                                    .p_0()
                                    .prefix(
                                        Icon::new(IconName::Search)
                                            .text_color(theme.muted_foreground),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .h(px(PALETTE_LIST_H))
                            .child(
                                div()
                                    .id("palette-results-scroll")
                                    .track_scroll(&self.results_scroll)
                                    .overflow_y_scroll()
                                    .size_full()
                                    .children({
                                        let results: Vec<_> = self
                                            .results
                                            .iter()
                                            .enumerate()
                                            .map(|(i, r)| {
                                                let is_sel = i == self.selected;
                                                let conn_label: SharedString =
                                                    r.conn_label.clone().into();
                                                let label: SharedString =
                                                    format::palette_single_line(&r.label, 120)
                                                        .into();
                                                let sublabel: SharedString =
                                                    r.sublabel.clone().into();
                                                (i, is_sel, conn_label, label, sublabel)
                                            })
                                            .collect();
                                        results.into_iter().map(
                                            |(i, is_sel, conn_label, label, sublabel)| {
                                                palette_result_row(
                                                    ("palette-result", i),
                                                    is_sel,
                                                    conn_label,
                                                    label,
                                                    sublabel,
                                                    muted,
                                                    fg,
                                                )
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        cx.stop_propagation();
                                                        this.selected = i;
                                                        this.open_selected(false, cx);
                                                    }),
                                                )
                                            },
                                        )
                                    }),
                            )
                            .child(Scrollbar::vertical(&self.results_scroll)),
                    )
                    .child(palette_footer_hints(window, cx)),
            )
            .into_any_element()
    }
}
