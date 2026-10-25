use serde_json::{Value, json};

/// Provider-neutral tool description (JSON Schema parameters).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

/// Errors are returned to the model as content so it can retry, not raised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

pub fn specs() -> Vec<ToolSpec> {
    let one = |key: &str, desc: &str| json!({ "type": "object", "properties": { key: { "type": "string", "description": desc } }, "required": [key] });
    vec![
        ToolSpec {
            name: "list_tables",
            description: "List tables, views or collections in the connected database.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "describe_table",
            description: "Show columns, types, keys and references for one table.",
            schema: one("table", "Table or collection name"),
        },
        ToolSpec {
            name: "explain",
            description: "Show the query plan for a read-only statement without executing it.",
            schema: one("query", "A read-only query"),
        },
        ToolSpec {
            name: "run_readonly_query",
            description: "Run a read-only query and return up to a limited number of rows.",
            schema: one("query", "A read-only query"),
        },
    ]
}
