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

// ---------------------------------------------------------------------------
// Schema inspector
// ---------------------------------------------------------------------------

/// Detailed description of a table or collection, rendered by the
/// schema inspector panel.
#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TableDescription {
    pub name: String,
    /// Schema name for Postgres; `None` for SQLite and MongoDB where
    /// schemas don't apply.
    pub schema: Option<String>,
    /// Engine-reported kind: `"table"`, `"view"`, `"collection"`, ...
    pub kind: String,
    pub columns: Vec<ColumnDescription>,
    pub indexes: Vec<IndexDescription>,
    pub foreign_keys: Vec<ForeignKeyDescription>,
    /// Estimated row count when the engine exposes one cheaply; `None`
    /// otherwise. We never issue `SELECT COUNT(*)` just to fill this
    /// in — the inspector is meant to be fast.
    pub row_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDescription {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub is_primary_key: bool,
    /// Ordinal position (1-indexed) in the original CREATE TABLE.
    pub position: i32,
}

#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexDescription {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
}

#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyDescription {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub referenced_schema: Option<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
}
