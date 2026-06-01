//! Per-tab editor context — shared state for query editors.
//!
//! [`EditorContext`] is a GPUI entity owned by each query editor panel.
//! It carries the connection scope, variable bindings, and schema cache.
//! Future LSP clients and autocomplete providers attach here.

use std::sync::Arc;

use based_core::EngineKind;
use gpui::EventEmitter;

use crate::connection::ConnectionId;

use super::{SchemaCache, VariableScope};

/// Events emitted when context state changes.
pub enum EditorContextEvent {
    VariablesChanged,
    SchemaCacheRefreshed,
}

/// Per-tab shared state for a query editor.
///
/// Create one per editor panel via `cx.new(|_| EditorContext::new(...))`.
/// Other components (autocomplete popup, explain overlay, lint runner) can
/// read this entity or subscribe to its events.
pub struct EditorContext {
    pub conn_id: ConnectionId,
    pub engine: EngineKind,
    pub variables: VariableScope,
    pub schema_cache: Arc<SchemaCache>,
}

impl EditorContext {
    pub fn new(conn_id: ConnectionId, engine: EngineKind, variables: VariableScope) -> Self {
        Self {
            schema_cache: Arc::new(SchemaCache::new(engine)),
            conn_id,
            engine,
            variables,
        }
    }

    /// Replace the variable scope (e.g. after `.env` reload).
    pub fn set_variables(&mut self, scope: VariableScope) {
        self.variables = scope;
    }

    /// Update the schema cache after a background refresh completes.
    pub fn set_schema_cache(&mut self, cache: SchemaCache) {
        self.schema_cache = Arc::new(cache);
    }
}

impl EventEmitter<EditorContextEvent> for EditorContext {}
