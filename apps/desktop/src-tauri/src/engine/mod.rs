//! Per-engine runtime operations.
//!
//! `connectors/` handles how the app **opens** a connection.
//! This module handles what the app does **through** an open connection:
//! listing objects, browsing tables / collections, and running raw
//! queries.
//!
//! Why a trait is coming but not here yet: the three engines share the
//! same `BrowseOptions`/`QueryResult` shape but take subtly different
//! parameters (SQLite has no schema, Postgres has schema + table,
//! Mongo has collection + find/aggregate). Rather than wedging a
//! single `EngineCapability` trait onto every caller today, we first
//! split each engine into its own module with identical function
//! signatures (`browse_table`, `execute_raw`, `value_to_json`) so the
//! eventual trait is an obvious `extract` refactor. Phase 1 todo 2
//! (Filter AST) and Phase 1 todo 3 (schema inspector) will add more
//! shared methods and that's when the trait emerges.

pub mod filters;
pub mod mongo;
pub mod mutations;
pub mod postgres;
pub mod sqlite;
pub mod types;
pub mod values;

pub use types::{BrowseOptions, QueryResult};
