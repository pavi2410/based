//! Inline EXPLAIN QUERY PLAN renderer shared by the SQLite query editor.

use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, prelude::*, px};
use gpui_component::{scroll::ScrollableElement, v_flex};

use super::eqp_parse::EqpNode;

/// Render a list of EQP roots as a scrollable indented tree.
pub fn render_eqp_body(
    id: impl Into<gpui::ElementId>,
    roots: &[EqpNode],
    theme: &gpui_component::Theme,
) -> impl IntoElement {
    let rows: Vec<AnyElement> = roots
        .iter()
        .map(|root| render_eqp_node(root, 0, theme))
        .collect();

    v_flex()
        .id(id)
        .w_full()
        .h_full()
        .overflow_y_scrollbar()
        .p(px(8.0))
        .children(rows)
}

fn render_eqp_node(node: &EqpNode, depth: usize, theme: &gpui_component::Theme) -> AnyElement {
    let warn = theme.warning;
    let fg = if node.is_table_scan {
        warn
    } else {
        theme.foreground
    };
    let row = div()
        .w_full()
        .py(px(2.0))
        .pl(px((depth * 16) as f32 + 8.0))
        .pr(px(8.0))
        .when(node.is_table_scan, |d| d.border_l_2().border_color(warn))
        .text_sm()
        .text_color(fg)
        .child(node.detail.clone());

    let children: Vec<AnyElement> = node
        .children
        .iter()
        .map(|c| render_eqp_node(c, depth + 1, theme))
        .collect();

    v_flex()
        .w_full()
        .child(row)
        .children(children)
        .into_any_element()
}
