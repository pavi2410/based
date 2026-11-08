//! EXPLAIN plan parsing (`based-postgres`) and GPUI rendering.

use gpui_kit::component::{Theme, h_flex, v_flex};
use gpui_kit::{AnyElement, IntoElement, ParentElement, Styled, div, prelude::*, px, relative};

pub use based_postgres::explain::{PlanNode, parse_pg_explain_json};

/// Slow-node threshold (ms) used to mark hot rows when timings are present.
pub const SLOW_MS: f64 = 100.0;
/// A node taking at least this share of the total cost is marked hot.
const HOT_SHARE: f64 = 0.4;
/// Sequential scans estimated above this many rows get an index hint.
const SEQ_SCAN_HINT_ROWS: u64 = 10_000;

/// One line of the flattened plan tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanRow {
    pub depth: usize,
    pub title: String,
    pub detail: String,
    /// This node's own cost as a fraction of the whole plan (children excluded).
    pub share: f64,
    pub self_cost: f64,
    pub time_ms: Option<f64>,
    pub hot: bool,
}

/// Flatten a plan into display rows plus plain-language hints.
pub fn plan_rows(root: &PlanNode) -> (Vec<PlanRow>, Vec<String>) {
    let total = root.cost_total.max(f64::EPSILON);
    let mut rows = Vec::new();
    let mut hints = Vec::new();
    fn walk(
        node: &PlanNode,
        depth: usize,
        total: f64,
        rows: &mut Vec<PlanRow>,
        hints: &mut Vec<String>,
    ) {
        let children_cost: f64 = node.children.iter().map(|c| c.cost_total).sum();
        let self_cost = (node.cost_total - children_cost).max(0.0);
        let share = (self_cost / total).clamp(0.0, 1.0);
        let relation = node
            .relation
            .as_deref()
            .map(|r| format!(" on {r}"))
            .unwrap_or_default();
        let index = node
            .index_name
            .as_deref()
            .map(|i| format!(" using {i}"))
            .unwrap_or_default();
        let detail = match node.rows_actual {
            Some(actual) => format!("{actual} rows (est {})", node.rows_estimated),
            None => format!("est {} rows", node.rows_estimated),
        };
        let hot = share >= HOT_SHARE || node.is_slow(SLOW_MS);
        if node.node_type == "Seq Scan" && node.rows_estimated >= SEQ_SCAN_HINT_ROWS {
            let table = node.relation.as_deref().unwrap_or("this table");
            hints.push(format!(
                "Sequential scan on {table} reads about {} rows. An index on the filtered column could help.",
                node.rows_estimated
            ));
        }
        rows.push(PlanRow {
            depth,
            title: format!("{}{}{}", node.node_type, relation, index),
            detail,
            share,
            self_cost,
            time_ms: node.time_actual_ms,
            hot,
        });
        for child in &node.children {
            walk(child, depth + 1, total, rows, hints);
        }
    }
    walk(root, 0, total, &mut rows, &mut hints);
    (rows, hints)
}

/// Render a plan as a tree: name, a bar for the node's share of the cost, and its number.
pub fn render_plan(root: &PlanNode, theme: &Theme) -> AnyElement {
    let (rows, hints) = plan_rows(root);
    let (muted, fg, warn, primary, border) = (
        theme.muted_foreground,
        theme.foreground,
        theme.warning,
        theme.primary,
        theme.border,
    );
    let has_time = rows.iter().any(|r| r.time_ms.is_some());
    let summary = if has_time {
        "Measured timings"
    } else {
        "Estimated cost from the planner. The query was not run."
    };

    v_flex()
        .w_full()
        .gap(px(4.0))
        .child(
            div()
                .px(px(8.0))
                .pb(px(4.0))
                .text_xs()
                .text_color(muted)
                .child(summary),
        )
        .children(rows.into_iter().map(|r| {
            let tone = if r.hot { warn } else { primary };
            let number = match r.time_ms {
                Some(t) => format!("{t:.1} ms"),
                None => format!("cost {:.0}", r.self_cost),
            };
            h_flex()
                .w_full()
                .gap(px(10.0))
                .items_center()
                .py(px(3.0))
                .pr(px(8.0))
                .pl(px(8.0 + r.depth as f32 * 16.0))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(6.0))
                        .items_baseline()
                        .when(r.depth > 0, |h| {
                            h.child(div().text_xs().text_color(muted).child("↳"))
                        })
                        .child(
                            div()
                                .text_xs()
                                .text_color(if r.hot { warn } else { fg })
                                .truncate()
                                .child(r.title),
                        )
                        .child(div().text_xs().text_color(muted).truncate().child(r.detail)),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .h(px(6.0))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(border.opacity(0.6))
                        .child(
                            div()
                                .h_full()
                                .w(relative(r.share.max(0.02) as f32))
                                .rounded_full()
                                .bg(tone),
                        ),
                )
                .child(
                    div()
                        .w(px(84.0))
                        .flex_shrink_0()
                        .text_right()
                        .text_xs()
                        .text_color(muted)
                        .child(number),
                )
        }))
        .children(hints.into_iter().map(|h| {
            div()
                .mt(px(8.0))
                .mx(px(8.0))
                .px(px(10.0))
                .py(px(6.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(warn.opacity(0.4))
                .bg(warn.opacity(0.08))
                .text_xs()
                .text_color(warn)
                .child(h)
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_simple_seq_scan() {
        let explain_output = json!([{
            "Plan": {
                "Node Type": "Seq Scan",
                "Relation Name": "users",
                "Startup Cost": 0.0,
                "Total Cost": 18.8,
                "Plan Rows": 1,
                "Actual Rows": 1,
                "Actual Total Time": 0.05,
                "Plans": []
            }
        }]);
        let node = parse_pg_explain_json(&explain_output).unwrap();
        assert_eq!(node.node_type, "Seq Scan");
        assert_eq!(node.relation.as_deref(), Some("users"));
        assert_eq!(node.time_actual_ms, Some(0.05));
    }

    #[test]
    fn detects_slow_node() {
        let node = PlanNode {
            node_type: "Seq Scan".into(),
            relation: None,
            index_name: None,
            cost_startup: 0.0,
            cost_total: 9999.0,
            rows_estimated: 1000,
            rows_actual: Some(1000),
            time_actual_ms: Some(500.0),
            children: vec![],
        };
        assert!(node.is_slow(100.0));
        assert!(!node.is_slow(1000.0));
    }

    fn node(kind: &str, total: f64, rows: u64, children: Vec<PlanNode>) -> PlanNode {
        PlanNode {
            node_type: kind.into(),
            relation: Some("users".into()),
            index_name: None,
            cost_startup: 0.0,
            cost_total: total,
            rows_estimated: rows,
            rows_actual: None,
            time_actual_ms: None,
            children,
        }
    }

    #[test]
    fn self_cost_excludes_children_and_marks_the_hot_node() {
        let tree = node(
            "Sort",
            1000.0,
            640,
            vec![node("Seq Scan", 900.0, 40_000, vec![])],
        );
        let (rows, hints) = plan_rows(&tree);
        assert_eq!(rows.len(), 2);
        assert!((rows[0].self_cost - 100.0).abs() < 1e-9);
        assert!(!rows[0].hot);
        assert!(rows[1].hot);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].contains("users"));
    }

    #[test]
    fn small_seq_scans_get_no_hint() {
        let (_, hints) = plan_rows(&node("Seq Scan", 10.0, 50, vec![]));
        assert!(hints.is_empty());
    }
}
