use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Provider-neutral tool description (JSON Schema parameters).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    const QUERY: &str = "A read-only query. SQL for Postgres/SQLite; for MongoDB a JSON object \
        {\"collection\": \"name\", \"pipeline\": [...]}; for Redis a single read command.";
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
            schema: one("query", QUERY),
        },
        ToolSpec {
            name: RUN_QUERY,
            description: "Run a read-only query and return up to a limited number of rows.",
            schema: one("query", QUERY),
        },
    ]
}

pub const LIST_CONNECTIONS: &str = "list_connections";
pub const CONNECT: &str = "connect";
pub const RUN_QUERY: &str = "run_readonly_query";

/// Tools for choosing the database; the app runs them, not a backend.
pub fn connection_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: LIST_CONNECTIONS,
            description: "List the project's database connections with engine, state and \
                whether the assistant may use them.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: CONNECT,
            description: "Connect to a database by connection name (opening it if needed) and \
                make it the one the other tools use.",
            schema: json!({ "type": "object", "properties": { "connection": { "type": "string", "description": "Connection name from list_connections" } }, "required": ["connection"] }),
        },
    ]
}
