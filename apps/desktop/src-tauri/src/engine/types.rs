//! Shared types used by every engine implementation and by the
//! command layer.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Result of a data query – rows plus column info plus an optional total
/// row count (for pagination UIs).
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub total_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
}

/// Paginated / filtered browse parameters. Consolidated so every engine
/// command takes exactly this type and stays under tauri-specta's
/// 10-argument command limit.
#[derive(Debug, Serialize, Deserialize, Clone, Default, Type)]
#[serde(rename_all = "camelCase")]
pub struct BrowseOptions {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub order_by_column: Option<String>,
    pub order_by_direction: Option<String>,
    /// JSON-encoded `Vec<FilterParam>`. Kept as a string (rather than a
    /// typed `Vec<FilterParam>`) so the UI's filter DSL can evolve
    /// without forcing a binding regen every time. Phase 1 todo 2
    /// replaces this with a proper `FilterAst` type.
    pub filters: Option<String>,
}

/// One row in the filter DSL sent from the UI. Matches the shape emitted
/// by `@/components/data-table-filter`.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
#[serde(rename_all = "camelCase")]
pub struct FilterParam {
    pub column_id: String,
    #[serde(rename = "type")]
    pub column_type: String,
    pub operator: String,
    pub values: Vec<serde_json::Value>,
}
