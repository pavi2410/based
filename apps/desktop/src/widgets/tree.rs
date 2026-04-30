// Tree — generic tree node primitive (label, icon, expand toggle, children).
// Each engine module uses this to render its schema/object tree.

use gpui::{prelude::*, *};
use gpui_component::IconName;

#[derive(Clone)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub icon: Option<IconName>,
    pub children: Vec<TreeNode>,
    pub expanded: bool,
}

impl TreeNode {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            children: vec![],
            expanded: false,
        }
    }

    pub fn with_icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_children(mut self, children: Vec<TreeNode>) -> Self {
        self.children = children;
        self
    }
}

/// Renders a flat list of visible nodes (respecting expanded state) using a
/// v_flex with indented rows.
pub struct TreeView {
    pub nodes: Vec<TreeNode>,
    pub selected_id: Option<String>,
}

impl TreeView {
    pub fn new(nodes: Vec<TreeNode>) -> Self {
        Self {
            nodes,
            selected_id: None,
        }
    }

    pub fn selected(mut self, id: Option<String>) -> Self {
        self.selected_id = id;
        self
    }

    fn visible_nodes(nodes: &[TreeNode], depth: usize, out: &mut Vec<(usize, TreeNode)>) {
        for node in nodes {
            out.push((depth, node.clone()));
            if node.expanded {
                Self::visible_nodes(&node.children, depth + 1, out);
            }
        }
    }
}

impl RenderOnce for TreeView {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut visible: Vec<(usize, TreeNode)> = vec![];
        Self::visible_nodes(&self.nodes, 0, &mut visible);

        let selected = self.selected_id.clone();

        let rows: Vec<_> = visible
            .into_iter()
            .map(|(depth, node)| {
                let is_selected = selected.as_deref() == Some(&node.id);
                let label: SharedString = node.label.clone().into();
                div()
                    .pl(px((depth * 16) as f32))
                    .py(px(2.0))
                    .px(px(8.0))
                    .when(is_selected, |d| d.bg(rgb(0x3b4261)))
                    .child(div().text_sm().child(label))
            })
            .collect();

        div().flex().flex_col().children(rows)
    }
}
