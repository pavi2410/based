//! Parse SQLite `EXPLAIN QUERY PLAN` rows into a tree.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EqpNode {
    pub id: i64,
    pub detail: String,
    pub children: Vec<EqpNode>,
    pub is_table_scan: bool,
}

pub fn parse_eqp(rows: &[(i64, i64, String)]) -> Vec<EqpNode> {
    let mut nodes: HashMap<i64, EqpNode> = rows
        .iter()
        .map(|(id, _, detail)| {
            let is_table_scan = detail.contains("SCAN") && !detail.contains("USING INDEX");
            (
                *id,
                EqpNode {
                    id: *id,
                    detail: detail.clone(),
                    children: vec![],
                    is_table_scan,
                },
            )
        })
        .collect();

    let mut roots = Vec::new();
    for (id, parent_id, _) in rows {
        if *parent_id == 0 {
            if let Some(node) = nodes.remove(id) {
                roots.push(node);
            }
        } else if let Some(child) = nodes.remove(id) {
            if let Some(parent) = nodes.get_mut(parent_id) {
                parent.children.push(child);
            } else {
                roots.push(child);
            }
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_scan_detected() {
        let rows = vec![(2, 0, "SCAN users".to_string())];
        let tree = parse_eqp(&rows);
        assert_eq!(tree.len(), 1);
        assert!(tree[0].is_table_scan);
    }

    #[test]
    fn index_scan_not_flagged() {
        let rows = vec![(2, 0, "SEARCH users USING INDEX idx_email".to_string())];
        let tree = parse_eqp(&rows);
        assert_eq!(tree.len(), 1);
        assert!(!tree[0].is_table_scan);
    }
}
