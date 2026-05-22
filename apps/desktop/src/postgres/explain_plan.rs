//! Parse PostgreSQL `EXPLAIN (FORMAT JSON)` output into a plan tree.

#[derive(Debug, Clone)]
pub struct PlanNode {
    pub node_type: String,
    pub relation: Option<String>,
    pub index_name: Option<String>,
    pub cost_startup: f64,
    pub cost_total: f64,
    pub rows_estimated: u64,
    pub rows_actual: Option<u64>,
    pub time_actual_ms: Option<f64>,
    pub children: Vec<PlanNode>,
}

impl PlanNode {
    pub fn is_slow(&self, threshold_ms: f64) -> bool {
        self.time_actual_ms
            .map(|t| t > threshold_ms)
            .unwrap_or(false)
    }
}

pub fn parse_pg_explain_json(json: &serde_json::Value) -> Option<PlanNode> {
    let plan = json.as_array()?.first()?.get("Plan")?;
    Some(parse_node(plan))
}

fn parse_node(node: &serde_json::Value) -> PlanNode {
    PlanNode {
        node_type: node["Node Type"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string(),
        relation: node["Relation Name"]
            .as_str()
            .map(|s| s.to_string()),
        index_name: node["Index Name"].as_str().map(|s| s.to_string()),
        cost_startup: node["Startup Cost"].as_f64().unwrap_or(0.0),
        cost_total: node["Total Cost"].as_f64().unwrap_or(0.0),
        rows_estimated: node["Plan Rows"].as_u64().unwrap_or(0),
        rows_actual: node["Actual Rows"].as_u64(),
        time_actual_ms: node["Actual Total Time"].as_f64(),
        children: node["Plans"]
            .as_array()
            .map(|plans| plans.iter().map(parse_node).collect())
            .unwrap_or_default(),
    }
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
}
