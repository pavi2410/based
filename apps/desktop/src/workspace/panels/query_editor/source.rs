//! The engine side of the query editor: what a SQL engine implements to plug in.

use based_core::{EngineKind, QueryError, SqlOutcome};
use futures::future::BoxFuture;
use gpui_kit::AnyElement;
use gpui_kit::component::Theme;

use crate::editor::SchemaCache;

pub type Fut<T> = BoxFuture<'static, anyhow::Result<T>>;

/// Why a run is waiting behind a confirmation banner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldReason {
    /// AI-written text that changes data, not yet run or edited.
    AiWrite,
    /// A data-changing statement on a connection tagged production.
    ProductionWrite,
    /// DROP, TRUNCATE, or UPDATE/DELETE without WHERE, on any connection.
    Destructive,
}

/// Engine-specific query plan, rendered inline.
pub trait PlanView: Send {
    fn render(&self, theme: &Theme) -> AnyElement;
}

pub enum Explained {
    Plan(Box<dyn PlanView>),
    Text(String),
}

/// Engine-specific execution for [`QueryEditorPanel`].
pub trait QuerySource: Send + Sync + 'static {
    /// Dock panel name, e.g. `"PgQueryEditor"`.
    fn panel_name(&self) -> &'static str;
    /// Element id prefix, e.g. `"pg"`.
    fn id(&self) -> &'static str;
    fn engine(&self) -> EngineKind;
    fn default_sql(&self) -> &'static str;
    /// Schema for completions; `None` disables completion.
    fn load_schema(&self) -> Option<Fut<SchemaCache>> {
        None
    }
    /// Run `sql`; an error's `position` is a byte offset into `sql`.
    fn run(
        &self,
        sql: String,
        timeout_secs: u32,
    ) -> BoxFuture<'static, Result<SqlOutcome, QueryError>>;
    fn explain(&self, sql: String) -> Fut<Explained>;
    /// Run `sql` where the database itself refuses writes (a read-only transaction,
    /// or SQLite's `query_only`), for SQL the assistant labelled read-only. At most
    /// `limit` rows (one more marks the cut).
    fn run_read_only(
        &self,
        sql: String,
        limit: usize,
    ) -> BoxFuture<'static, Result<SqlOutcome, QueryError>>;
}
