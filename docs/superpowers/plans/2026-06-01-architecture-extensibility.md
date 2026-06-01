# Architecture Extensibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the Based desktop codebase through six incremental priorities that reduce O(engines×tab_types) dispatch, add typed auth and query payloads, establish an editor context service, and introduce tab scoping — enabling new DB engines, auth methods, tab types, and editor features to be added without touching central dispatch files.

**Architecture:** Each priority is self-contained and compiles after every commit. Priorities 1–2 decouple engine dispatch; Priority 3 types the query editor payload; Priority 4 adds a typed auth model; Priority 5 establishes an editor context service; Priority 6 introduces tab scoping. All changes are Rust-idiomatic, GPUI-compatible, and backward-compatible with existing `.based/` files and session snapshots.

**Tech Stack:** Rust, GPUI (Zed), gpui-component DockArea, sqlx (PgPool/SqlitePool), mongodb driver, serde JSON for session snapshots.

**Workspace:** `/Users/pavi2410/Projects/pavi2410/based` — branch `feat/architecture-extensibility`.

**Build commands:**
- Check: `cargo check -p desktop`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Test: `cargo test --workspace`
- Format: `cargo fmt --all`

---

## Context for implementers

Key files to understand before any task:

- `apps/desktop/src/connection/mod.rs` — `ConnectionConfig` enum (Postgres/MongoDB/SQLite variants), `AnyConnection` enum
- `apps/desktop/src/workspace/tabs/spec.rs` — `TabSpec` enum (all tab identity types)
- `apps/desktop/src/workspace/tabs/dispatch.rs` — `dispatch_open_tab()` — the O(engines×tab_types) match-of-matches being fixed
- `apps/desktop/src/connection/lifecycle.rs` — `Connectable` trait (open/test/close)
- `apps/desktop/src/main.rs` — app startup, global registration
- `crates/based-core/src/engine.rs` — `EngineKind` enum
- `crates/based-core/src/lib.rs` — re-exports from based-core

The current `dispatch_open_tab` is a nested match: outer on `TabSpec` variant, inner on `AnyConnection` variant. Every new engine requires adding inner arms to every outer branch. Every new tab type requires a new outer branch with its own inner arms for each engine.

---

## Task 1: EngineDescriptor trait and EngineRegistry global (Priority 1)

**Goal:** Introduce an `EngineDescriptor` trait that each engine module implements, and an `EngineRegistry` global that collects them at startup. Purely additive — no existing behavior changes.

**Files:**
- Create: `apps/desktop/src/connection/descriptor.rs`
- Modify: `apps/desktop/src/connection/mod.rs` (add `pub mod descriptor; pub use descriptor::*;`)
- Modify: `apps/desktop/src/postgres/mod.rs` (add `pub struct PostgresEngine; impl EngineDescriptor`)
- Modify: `apps/desktop/src/sqlite/mod.rs` (add `pub struct SqliteEngine; impl EngineDescriptor`)
- Modify: `apps/desktop/src/mongodb/mod.rs` (add `pub struct MongoEngine; impl EngineDescriptor`)
- Modify: `apps/desktop/src/main.rs` (register engines into `EngineRegistry`)

- [ ] **Step 1: Create `descriptor.rs`**

```rust
// apps/desktop/src/connection/descriptor.rs
//! Engine descriptor trait and registry.
//!
//! Each engine module exports a zero-size struct implementing [`EngineDescriptor`].
//! The registry is a GPUI global populated at startup — new engines register here
//! without touching any central dispatch file.

use based_core::EngineKind;
use gpui::Global;

/// Metadata describing a database engine family.
///
/// Implement this for each engine. The registry uses it for display, capability
/// queries, and (in P2) tab panel construction.
pub trait EngineDescriptor: Send + Sync + 'static {
    fn kind(&self) -> EngineKind;
    fn display_name(&self) -> &str;
    /// Short icon identifier matched by the theme asset loader (e.g. `"postgres"`, `"sqlite"`).
    fn icon_name(&self) -> &str;
    /// Default TCP port for new connection forms, `None` for file-based engines.
    fn default_port(&self) -> Option<u16>;
    /// Whether this engine supports a given tab kind label (used for feature gating UI).
    fn supports_tab_kind(&self, kind: &str) -> bool;
}

/// App-level registry of all registered engine descriptors.
///
/// Populated at startup via [`EngineRegistry::register`]; read everywhere via
/// `cx.try_global::<EngineRegistry>()`.
pub struct EngineRegistry {
    descriptors: Vec<Box<dyn EngineDescriptor>>,
}

impl Global for EngineRegistry {}

impl EngineRegistry {
    pub fn new() -> Self {
        Self {
            descriptors: vec![],
        }
    }

    pub fn register(&mut self, descriptor: impl EngineDescriptor) {
        self.descriptors.push(Box::new(descriptor));
    }

    pub fn find(&self, kind: EngineKind) -> Option<&dyn EngineDescriptor> {
        self.descriptors
            .iter()
            .find(|d| d.kind() == kind)
            .map(|d| d.as_ref())
    }

    pub fn all(&self) -> &[Box<dyn EngineDescriptor>] {
        &self.descriptors
    }

    pub fn display_name(&self, kind: EngineKind) -> &str {
        self.find(kind).map(|d| d.display_name()).unwrap_or("Unknown")
    }

    pub fn icon_name(&self, kind: EngineKind) -> &str {
        self.find(kind).map(|d| d.icon_name()).unwrap_or("")
    }
}
```

- [ ] **Step 2: Add `pub mod descriptor; pub use descriptor::*;` to `connection/mod.rs`**

Add these two lines after the existing `pub mod lifecycle;` block (around line 10):
```rust
pub mod descriptor;
pub use descriptor::{EngineDescriptor, EngineRegistry};
```

- [ ] **Step 3: Add `PostgresEngine` to `postgres/mod.rs`**

Add at the bottom of `apps/desktop/src/postgres/mod.rs`:
```rust
use crate::connection::descriptor::EngineDescriptor;
use based_core::EngineKind;

/// Engine descriptor for PostgreSQL — registered at startup via [`EngineRegistry`].
pub struct PostgresEngine;

impl EngineDescriptor for PostgresEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::Postgres
    }

    fn display_name(&self) -> &str {
        "PostgreSQL"
    }

    fn icon_name(&self) -> &str {
        "postgres"
    }

    fn default_port(&self) -> Option<u16> {
        Some(5432)
    }

    fn supports_tab_kind(&self, kind: &str) -> bool {
        matches!(
            kind,
            "query_editor" | "data_viewer" | "inspector" | "object_info" | "dashboard"
        )
    }
}
```

- [ ] **Step 4: Add `SqliteEngine` to `sqlite/mod.rs`**

First read `apps/desktop/src/sqlite/mod.rs` to find the right insertion point, then add:
```rust
use crate::connection::descriptor::EngineDescriptor;
use based_core::EngineKind;

/// Engine descriptor for SQLite — registered at startup via [`EngineRegistry`].
pub struct SqliteEngine;

impl EngineDescriptor for SqliteEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::SQLite
    }

    fn display_name(&self) -> &str {
        "SQLite"
    }

    fn icon_name(&self) -> &str {
        "sqlite"
    }

    fn default_port(&self) -> Option<u16> {
        None
    }

    fn supports_tab_kind(&self, kind: &str) -> bool {
        matches!(
            kind,
            "query_editor" | "data_viewer" | "inspector" | "object_info" | "dashboard"
        )
    }
}
```

- [ ] **Step 5: Add `MongoEngine` to `mongodb/mod.rs`**

Read `apps/desktop/src/mongodb/mod.rs` to find the right insertion point, then add:
```rust
use crate::connection::descriptor::EngineDescriptor;
use based_core::EngineKind;

/// Engine descriptor for MongoDB — registered at startup via [`EngineRegistry`].
pub struct MongoEngine;

impl EngineDescriptor for MongoEngine {
    fn kind(&self) -> EngineKind {
        EngineKind::MongoDB
    }

    fn display_name(&self) -> &str {
        "MongoDB"
    }

    fn icon_name(&self) -> &str {
        "mongodb"
    }

    fn default_port(&self) -> Option<u16> {
        Some(27017)
    }

    fn supports_tab_kind(&self, kind: &str) -> bool {
        matches!(
            kind,
            "query_editor" | "pipeline" | "data_viewer" | "inspector" | "document_insert"
                | "dashboard"
        )
    }
}
```

- [ ] **Step 6: Register engines in `main.rs`**

After `db::init(cx);` in `main.rs`, add:
```rust
// Engine registry — add new engines here; no other files need to change.
{
    let mut registry = crate::connection::EngineRegistry::new();
    registry.register(crate::postgres::PostgresEngine);
    registry.register(crate::sqlite::SqliteEngine);
    registry.register(crate::mongodb::MongoEngine);
    cx.set_global(registry);
}
```

- [ ] **Step 7: Verify it compiles**

```bash
cargo check -p desktop 2>&1 | head -40
```
Expected: no errors (warnings about unused imports are fine).

- [ ] **Step 8: Commit**

```bash
cd /Users/pavi2410/Projects/pavi2410/based
cargo fmt --all
git add apps/desktop/src/connection/descriptor.rs \
        apps/desktop/src/connection/mod.rs \
        apps/desktop/src/postgres/mod.rs \
        apps/desktop/src/sqlite/mod.rs \
        apps/desktop/src/mongodb/mod.rs \
        apps/desktop/src/main.rs
git commit -m "$(cat <<'EOF'
refactor: add EngineDescriptor trait and EngineRegistry global (P1)

Introduces a registration pattern for database engine families.
Each engine exports a zero-size EngineDescriptor implementor; the
EngineRegistry global is populated at startup. Adding a new engine
family no longer requires touching central dispatch — only a new
register() call in main.rs.

Purely additive: no existing behavior changes.
EOF
)"
```

---

## Task 2: Per-engine tab dispatch functions (Priority 2)

**Goal:** Extract each engine's panel-creation logic from the monolithic `dispatch_open_tab` into per-engine `build_panel` free functions. The central dispatcher becomes a short loop/delegate. Adding a new tab type no longer requires editing `dispatch_open_tab`.

**Files:**
- Create: `apps/desktop/src/postgres/tab_dispatch.rs`
- Create: `apps/desktop/src/sqlite/tab_dispatch.rs`
- Create: `apps/desktop/src/mongodb/tab_dispatch.rs`
- Modify: `apps/desktop/src/postgres/mod.rs` (add `pub mod tab_dispatch;`)
- Modify: `apps/desktop/src/sqlite/mod.rs` (add `pub mod tab_dispatch;`)
- Modify: `apps/desktop/src/mongodb/mod.rs` (add `pub mod tab_dispatch;`)
- Modify: `apps/desktop/src/workspace/tabs/dispatch.rs` (replace match-of-matches with delegation)

**Key constraint:** `build_panel` is a free function, NOT stored as a trait object — this avoids GPUI lifetime issues with `Context<Workspace>`. The outer `match ac {}` in `dispatch_open_tab` remains (one short arm per engine), but each arm delegates to its engine's `build_panel`. Adding a new tab type only requires editing the relevant engine's `tab_dispatch.rs`.

- [ ] **Step 1: Read current dispatch.rs fully**

Read `apps/desktop/src/workspace/tabs/dispatch.rs` lines 1–327. Extract the Postgres block (lines ~51–94 for DataViewer, ~132–144 for QueryEditor, ~224–235 for Inspector), the SQLite block, and the MongoDB block.

- [ ] **Step 2: Create `postgres/tab_dispatch.rs`**

```rust
// apps/desktop/src/postgres/tab_dispatch.rs
//! Panel construction for PostgreSQL tabs.
//!
//! [`build_panel`] is the single dispatch point for all Postgres tab types.
//! Add new Postgres tabs here; `dispatch_open_tab` in workspace does not need
//! to change.

use std::sync::Arc;

use gpui::{Context, Window};
use gpui_component::dock::PanelView;
use sqlx::PgPool;

use crate::connection::ConnectionId;
use crate::workspace::{Workspace, tabs::spec::TabSpec};

/// Try to build a Postgres panel for `spec`.
///
/// Returns `None` when `spec` is a tab kind this engine doesn't handle (e.g.
/// `Home`, `ReleaseNotes`). The caller falls through to the next engine.
pub fn build_panel(
    spec: &TabSpec,
    pool: PgPool,
    conn_id: &ConnectionId,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Option<Arc<dyn PanelView>> {
    match spec {
        TabSpec::DataViewer { object, .. } => {
            let (schema, name) = match object.rsplit_once('.') {
                Some((s, n)) if !n.is_empty() => (s.to_string(), n.to_string()),
                _ => ("public".to_string(), object.clone()),
            };
            let panel = cx.new(|cx| {
                super::data_viewer::DataViewerPanel::new(pool, schema, name, window, cx)
            });
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor {
            initial_sql,
            auto_run,
            ..
        } => {
            let panel = cx.new(|cx| {
                super::query_editor::QueryEditorPanel::new_with_initial(
                    pool,
                    conn_id.clone(),
                    initial_sql.clone(),
                    *auto_run,
                    window,
                    cx,
                )
            });
            Some(Arc::new(panel))
        }
        TabSpec::Inspector { object, .. } => {
            let (schema, name) = match object.rsplit_once('.') {
                Some((s, n)) if !n.is_empty() => (s.to_string(), n.to_string()),
                _ => ("public".to_string(), object.clone()),
            };
            let panel = cx.new(|cx| {
                super::inspector::TableInspectorPanel::new(pool, schema, name, window, cx)
            });
            Some(Arc::new(panel))
        }
        _ => None,
    }
}
```

- [ ] **Step 3: Create `sqlite/tab_dispatch.rs`**

Read `apps/desktop/src/workspace/tabs/dispatch.rs` SQLite arms (DataViewer, QueryEditor, Inspector) and extract them into:

```rust
// apps/desktop/src/sqlite/tab_dispatch.rs
//! Panel construction for SQLite tabs.

use std::sync::Arc;

use gpui::{Context, Window};
use gpui_component::dock::PanelView;
use sqlx::SqlitePool;

use crate::connection::ConnectionId;
use crate::workspace::{Workspace, tabs::spec::TabSpec};

pub fn build_panel(
    spec: &TabSpec,
    pool: SqlitePool,
    conn_id: &ConnectionId,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Option<Arc<dyn PanelView>> {
    match spec {
        TabSpec::DataViewer { object, .. } => {
            let panel = cx.new(|cx| {
                super::data_viewer::DataViewerPanel::new(pool, object.clone(), window, cx)
            });
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor {
            initial_sql,
            auto_run,
            ..
        } => {
            let panel = cx.new(|cx| {
                super::query_editor::QueryEditorPanel::new_with_initial(
                    pool,
                    conn_id.clone(),
                    initial_sql.clone(),
                    *auto_run,
                    window,
                    cx,
                )
            });
            Some(Arc::new(panel))
        }
        TabSpec::Inspector { object, .. } => {
            let panel = cx.new(|cx| {
                super::inspector::TableInspectorPanel::new(pool, object.clone(), window, cx)
            });
            Some(Arc::new(panel))
        }
        _ => None,
    }
}
```

- [ ] **Step 4: Create `mongodb/tab_dispatch.rs`**

Read the MongoDB arms from `dispatch.rs` and extract:

```rust
// apps/desktop/src/mongodb/tab_dispatch.rs
//! Panel construction for MongoDB tabs.

use std::sync::Arc;

use ::mongodb::bson::Document;
use gpui::{Context, Window};
use gpui_component::dock::PanelView;
use mongodb::{Collection, Database};

use crate::connection::ConnectionId;
use crate::workspace::{Workspace, tabs::spec::TabSpec};

pub fn build_panel(
    spec: &TabSpec,
    db: Database,
    conn_id: &ConnectionId,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Option<Arc<dyn PanelView>> {
    match spec {
        TabSpec::DataViewer { object, .. } => {
            let collection: Collection<Document> = db.collection(object);
            let panel = cx.new(|cx| {
                super::document_viewer::DocumentViewerPanel::new(collection, window, cx)
            });
            Some(Arc::new(panel))
        }
        TabSpec::QueryEditor {
            initial_sql,
            initial_pipeline,
            mongo_collection,
            conn_id: spec_conn_id,
            auto_run,
        } => {
            let coll_name = mongo_collection
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("based_explorer");
            let coll: Collection<Document> = db.collection(coll_name);
            let merged = initial_pipeline.clone().or_else(|| initial_sql.clone());
            let panel = cx.new(|cx| {
                super::pipeline_builder::PipelineBuilderPanel::new_with_pipeline(
                    coll,
                    conn_id.clone(),
                    merged,
                    window,
                    cx,
                )
            });
            Some(Arc::new(panel))
        }
        TabSpec::Pipeline { collection, .. } => {
            let coll: Collection<Document> = db.collection(collection);
            let panel = cx.new(|cx| {
                super::pipeline_builder::PipelineBuilderPanel::new(
                    coll,
                    conn_id.clone(),
                    window,
                    cx,
                )
            });
            Some(Arc::new(panel))
        }
        TabSpec::Inspector { object, .. } => {
            let coll: Collection<Document> = db.collection(object);
            let panel = cx.new(|cx| {
                super::inspector::CollectionInspectorPanel::new(coll, window, cx)
            });
            Some(Arc::new(panel))
        }
        TabSpec::DocumentInsert { collection, .. } => {
            let coll: Collection<Document> = db.collection(collection);
            let panel = cx.new(|cx| {
                super::document_editor::DocumentEditorPanel::new_insert(coll, window, cx)
            });
            Some(Arc::new(panel))
        }
        _ => None,
    }
}
```

- [ ] **Step 5: Declare the new submodules in each engine `mod.rs`**

In `postgres/mod.rs`, add: `pub mod tab_dispatch;`
In `sqlite/mod.rs`, add: `pub mod tab_dispatch;`
In `mongodb/mod.rs`, add: `pub mod tab_dispatch;`

- [ ] **Step 6: Rewrite `dispatch_open_tab` to delegate**

Replace the body of `Workspace::dispatch_open_tab` in `apps/desktop/src/workspace/tabs/dispatch.rs`.

The new structure:

```rust
impl Workspace {
    fn find_connection(&self, id: &ConnectionId, cx: &App) -> Option<Entity<ConnectionEntry>> {
        // same as before
    }

    pub(crate) fn dispatch_open_tab(
        &mut self,
        spec: TabSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Connection-independent tabs handled first.
        match &spec {
            TabSpec::Home => {
                self.show_home(window, cx);
                return;
            }
            TabSpec::ReleaseNotes { version } => {
                let tab_spec = spec.clone();
                let panel = cx.new(|cx| {
                    crate::workspace::panels::release_notes::ReleaseNotesPanel::new(
                        version.clone(),
                        window,
                        cx,
                    )
                });
                self.dock_add_and_register_tab(tab_spec, Arc::new(panel), window, cx);
                return;
            }
            TabSpec::ObjectInfo {
                object_name,
                kind_label,
                ..
            } => {
                use crate::workspace::panels::object_info::ObjectInfoPanel;
                let tab_spec = spec.clone();
                let panel = cx.new(|cx| {
                    ObjectInfoPanel::new(object_name.clone(), kind_label.clone(), window, cx)
                });
                self.dock_add_and_register_tab(tab_spec, Arc::new(panel), window, cx);
                return;
            }
            TabSpec::Dashboard(conn_id) => {
                self.connection_tree.update(cx, |tree, ecx| {
                    tree.focus_connection_by_id(conn_id, ecx);
                });
                return;
            }
            TabSpec::Builtin { .. } => return,
            _ => {}
        }

        // Connection-scoped tabs: resolve the live connection.
        let conn_id = spec.conn_id().clone();
        let Some(ent) = self.find_connection(&conn_id, cx) else {
            return;
        };
        let ac = match &ent.read(cx).state {
            ConnectionState::Connected(ac) => ac.clone(),
            _ => return,
        };

        // Delegate panel construction to each engine's tab_dispatch module.
        // Adding a new engine: add one arm here + create its tab_dispatch.rs.
        // Adding a new tab type for an existing engine: edit that engine's tab_dispatch.rs only.
        let panel = match ac {
            AnyConnection::Postgres(conn) => {
                let pool = conn.read(cx).pool.clone();
                crate::postgres::tab_dispatch::build_panel(&spec, pool, &conn_id, window, cx)
            }
            AnyConnection::SQLite(conn) => {
                let pool = conn.read(cx).pool.clone();
                crate::sqlite::tab_dispatch::build_panel(&spec, pool, &conn_id, window, cx)
            }
            AnyConnection::MongoDB(conn) => {
                let db = conn.read(cx).database().clone();
                crate::mongodb::tab_dispatch::build_panel(&spec, db, &conn_id, window, cx)
            }
        };

        if let Some(panel) = panel {
            let tab_label = super::label::tab_label_for_spec(&spec, false);
            // Set tab label on the panel entity if it exposes one.
            // (The register_dock_panel! macro previously did this inline.)
            self.dock_add_and_register_tab(spec, panel, window, cx);
        }
    }

    fn dock_add_and_register_tab(/* same signature as before */) { /* unchanged */ }
}
```

IMPORTANT: The `tab_label` is set via the `register_dock_panel!` macro. Check how that macro works vs the new approach — the macro called `panel.update(cx, |p, _| p.tab_label = label)` which requires knowing the panel's concrete type. In the new approach we have `Arc<dyn PanelView>`. Check if `PanelView` exposes a way to set the label, or if we need to set it inside each `build_panel` function before returning.

After checking the macro: if label setting requires concrete type, move the `tab_label = label` assignment into each engine's `build_panel` before wrapping in `Arc::new(...)`. If `PanelView` exposes a label setter, call it on the `Arc<dyn PanelView>`.

Read the macro at lines 17–27 of `dispatch.rs` to confirm the approach.

- [ ] **Step 7: Verify compilation**

```bash
cargo check -p desktop 2>&1 | head -60
```
Expected: no errors. Fix any type mismatches.

- [ ] **Step 8: Run tests**

```bash
cargo test --workspace 2>&1 | tail -20
```
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
cd /Users/pavi2410/Projects/pavi2410/based
cargo fmt --all
git add apps/desktop/src/postgres/tab_dispatch.rs \
        apps/desktop/src/sqlite/tab_dispatch.rs \
        apps/desktop/src/mongodb/tab_dispatch.rs \
        apps/desktop/src/postgres/mod.rs \
        apps/desktop/src/sqlite/mod.rs \
        apps/desktop/src/mongodb/mod.rs \
        apps/desktop/src/workspace/tabs/dispatch.rs
git commit -m "$(cat <<'EOF'
refactor: extract per-engine tab dispatch functions (P2)

Each engine module now owns its panel-construction logic in
tab_dispatch::build_panel(). The central dispatch_open_tab becomes a
short delegating match with one arm per engine.

Adding a new tab type for an existing engine now only requires editing
that engine's tab_dispatch.rs — the central dispatcher is stable.
Adding a new engine requires one new match arm + a new tab_dispatch.rs.
EOF
)"
```

---

## Task 3: Typed QueryEditorInit payload (Priority 3)

**Goal:** Replace the flat, mixed-engine fields on `TabSpec::QueryEditor` with a typed `QueryEditorInit` enum. Serde backward compatibility is preserved via a custom deserializer shim that reads both old and new formats.

**Problem statement (current):**
```rust
TabSpec::QueryEditor {
    conn_id, initial_sql, initial_pipeline, auto_run, mongo_collection
}
```
`initial_pipeline` and `mongo_collection` are MongoDB-only but live in the shared type. Every engine adding init fields pollutes this enum.

**Target:**
```rust
pub enum QueryEditorInit {
    Sql { sql: Option<String>, auto_run: bool },
    MongoPipeline { pipeline: Option<String>, collection: String },
}

TabSpec::QueryEditor { conn_id, init: QueryEditorInit }
```

**Files:**
- Modify: `apps/desktop/src/workspace/tabs/spec.rs` — add `QueryEditorInit`, restructure variant, add serde compat
- Modify: `apps/desktop/src/workspace/project_query.rs` — update `tab_spec_for_query`
- Modify: `apps/desktop/src/postgres/tab_dispatch.rs` — update pattern matching
- Modify: `apps/desktop/src/sqlite/tab_dispatch.rs` — update pattern matching
- Modify: `apps/desktop/src/mongodb/tab_dispatch.rs` — update pattern matching
- Modify: `apps/desktop/src/workspace/tabs/label.rs` — update `tab_label_for_spec`
- Modify: `apps/desktop/src/workspace/tabs/manager.rs` — update `is_query` check if needed
- Modify: `apps/desktop/src/workspace/tabs/infer.rs` — update `infer_tab_spec`
- Modify: `apps/desktop/src/workspace/tabs/open.rs` — update construction sites
- Modify: `apps/desktop/src/workspace/tabs/commands.rs` — update any constructions
- Modify: `apps/desktop/src/workspace/query_lane.rs` — update construction
- Modify: `apps/desktop/src/command_palette/selection.rs` — update construction
- Modify: `apps/desktop/src/command_palette/search.rs` — update construction
- Modify: `apps/desktop/src/workspace/chrome/panes/history_pane.rs` — update construction
- Modify: `apps/desktop/src/workspace/mod.rs` — update construction

- [ ] **Step 1: Find all construction sites**

```bash
grep -rn "TabSpec::QueryEditor" apps/desktop/src/ --include="*.rs"
```
List every file and line. These are all the sites that need updating.

- [ ] **Step 2: Add `QueryEditorInit` and update `spec.rs`**

Replace the `QueryEditor` variant and add the `QueryEditorInit` type. Preserve serde compat using a `#[serde(from = "QueryEditorRaw")]` shim for deserialization, while serializing the new format.

```rust
// In spec.rs — add QueryEditorInit type
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryEditorInit {
    /// SQL query editor (Postgres, SQLite).
    Sql {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sql: Option<String>,
        #[serde(default)]
        auto_run: bool,
    },
    /// MongoDB aggregation pipeline editor.
    MongoPipeline {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pipeline: Option<String>,
        collection: String,
    },
}

impl Default for QueryEditorInit {
    fn default() -> Self {
        Self::Sql {
            sql: None,
            auto_run: true,
        }
    }
}
```

For the `TabSpec::QueryEditor` variant:
```rust
/// Query/pipeline editor tab. QueryEditor always opens a new tab (see TabManager).
QueryEditor {
    conn_id: ConnectionId,
    #[serde(flatten)]
    init: QueryEditorInit,
},
```

Wait — `#[serde(flatten)]` on an enum variant inside another enum with `#[serde(tag = "type")]` may cause issues. Instead, use a separate serde representation:

```rust
// Internal type for serde migration
#[derive(Deserialize)]
struct QueryEditorSpecCompat {
    conn_id: ConnectionId,
    // new format
    #[serde(default)]
    init: Option<QueryEditorInit>,
    // old flat format (v1 sessions)
    #[serde(default)]
    initial_sql: Option<String>,
    #[serde(default)]
    initial_pipeline: Option<String>,
    #[serde(default = "default_auto_run")]
    auto_run: bool,
    #[serde(default)]
    mongo_collection: Option<String>,
}

fn default_auto_run() -> bool { true }

impl From<QueryEditorSpecCompat> for (ConnectionId, QueryEditorInit) {
    fn from(c: QueryEditorSpecCompat) -> Self {
        let init = c.init.unwrap_or_else(|| {
            if c.initial_pipeline.is_some() || c.mongo_collection.is_some() {
                QueryEditorInit::MongoPipeline {
                    pipeline: c.initial_pipeline,
                    collection: c.mongo_collection.unwrap_or_default(),
                }
            } else {
                QueryEditorInit::Sql {
                    sql: c.initial_sql,
                    auto_run: c.auto_run,
                }
            }
        });
        (c.conn_id, init)
    }
}
```

Then `TabSpec` implements a custom `Deserialize` that handles both formats, OR we use `#[serde(try_from = "TabSpecRaw")]` for the whole enum.

The simplest approach given the `#[serde(tag = "type")]` on `TabSpec`: implement custom deserialization only for the `query_editor` case. Use a two-pass approach: deserialize with both old and new fields present, then normalize.

Implementation: Add the compat struct with `#[serde(skip)]` on `init` when old fields are present, and use `fn migrate_query_editor` called during session load:

```rust
// In session.rs, after loading TabSpec vec, call:
fn migrate_tab_spec(spec: TabSpec) -> TabSpec { spec } // identity for now — migration handled by serde compat
```

The most robust approach for this codebase: Keep `TabSpec::QueryEditor` with the old serde field names on the wire (for now), but rename the Rust struct fields internally using `#[serde(rename)]`:

```rust
QueryEditor {
    conn_id: ConnectionId,
    #[serde(rename = "initial_sql", default, skip_serializing_if = "Option::is_none")]
    // MIGRATION NOTE: "init" becomes the field name once all clients are on new format
    init: QueryEditorInit,
}
```

Actually the cleanest migration path: keep existing wire format but wrap into `QueryEditorInit` internally. The old fields become the serialization of the new enum. Use a custom serialize/deserialize via `#[serde(with = "query_editor_init_compat")]`:

For simplicity of implementation, use the approach below. Serialize `QueryEditorInit` using the old flat field names but deserialize from either format:

```rust
// Don't use serde(tag) on QueryEditorInit — serialize flat fields manually
// instead, and rely on the presence/absence of fields to distinguish:
impl Serialize for QueryEditorInit { ... }  // writes flat fields: initial_sql/initial_pipeline/etc.
impl Deserialize for QueryEditorInit { ... }  // reads either init:{kind:...} or flat fields
```

IMPLEMENTATION GUIDANCE: After reading all the construction sites in Step 1, use whichever serde compat approach is simplest. The priority is correctness and compilation. The wire format can be cleaned up in a follow-up.

- [ ] **Step 3: Update `blank_query_editor` and other constructors in `spec.rs`**

```rust
pub fn blank_query_editor(conn_id: ConnectionId) -> Self {
    Self::QueryEditor {
        conn_id,
        init: QueryEditorInit::default(),
    }
}
```

- [ ] **Step 4: Update `project_query.rs`**

```rust
pub fn tab_spec_for_query(query: &ProjectQuery, conn_id: ConnectionId) -> TabSpec {
    match &query.body {
        QueryBody::Sql { query } => TabSpec::QueryEditor {
            conn_id,
            init: QueryEditorInit::Sql {
                sql: Some(query.clone()),
                auto_run: false,
            },
        },
        QueryBody::Aggregate { pipeline, collection } => TabSpec::QueryEditor {
            conn_id,
            init: QueryEditorInit::MongoPipeline {
                pipeline: Some(pipeline.clone()),
                collection: collection.clone().unwrap_or_default(),
            },
        },
    }
}
```

- [ ] **Step 5: Update `postgres/tab_dispatch.rs`**

Change the `QueryEditor` arm to destructure `init`:
```rust
TabSpec::QueryEditor { init: QueryEditorInit::Sql { sql, auto_run }, .. } => {
    let panel = cx.new(|cx| {
        super::query_editor::QueryEditorPanel::new_with_initial(
            pool,
            conn_id.clone(),
            sql.clone(),
            *auto_run,
            window,
            cx,
        )
    });
    Some(Arc::new(panel))
}
TabSpec::QueryEditor { init: QueryEditorInit::MongoPipeline { .. }, .. } => None,
```

- [ ] **Step 6: Update `sqlite/tab_dispatch.rs`** (same pattern as Step 5)

- [ ] **Step 7: Update `mongodb/tab_dispatch.rs`**

```rust
TabSpec::QueryEditor { init: QueryEditorInit::MongoPipeline { pipeline, collection }, .. } => {
    let coll: Collection<Document> = db.collection(collection);
    let panel = cx.new(|cx| {
        super::pipeline_builder::PipelineBuilderPanel::new_with_pipeline(
            coll,
            conn_id.clone(),
            pipeline.clone(),
            window,
            cx,
        )
    });
    Some(Arc::new(panel))
}
TabSpec::QueryEditor { init: QueryEditorInit::Sql { .. }, .. } => None,
```

- [ ] **Step 8: Update all remaining construction sites**

For each file from Step 1 that constructs `TabSpec::QueryEditor`:
```rust
// Old:
TabSpec::QueryEditor {
    conn_id,
    initial_sql: Some(sql.clone()),
    initial_pipeline: None,
    auto_run: false,
    mongo_collection: None,
}
// New:
TabSpec::QueryEditor {
    conn_id,
    init: QueryEditorInit::Sql { sql: Some(sql.clone()), auto_run: false },
}
```

- [ ] **Step 9: Verify compilation**

```bash
cargo check -p desktop 2>&1 | head -60
```

- [ ] **Step 10: Run tests**

```bash
cargo test --workspace 2>&1 | tail -20
```

- [ ] **Step 11: Commit**

```bash
cd /Users/pavi2410/Projects/pavi2410/based
cargo fmt --all
git add -A
git commit -m "$(cat <<'EOF'
refactor: typed QueryEditorInit payload for QueryEditor tab (P3)

Replaces the flat, mixed-engine fields on TabSpec::QueryEditor with a
typed QueryEditorInit enum (Sql | MongoPipeline). Engine-specific init
data no longer pollutes the shared TabSpec type.

Serde migration shim preserves session restore from old flat format.
EOF
)"
```

---

## Task 4: AuthMethod type in based-core (Priority 4)

**Goal:** Add a typed `AuthMethod` enum to `crates/based-core` that will serve as the future home for all auth variants (password, SSH tunnel, IAM, mTLS, OAuth). Purely additive — not yet wired into `ConnectionConfig` (that requires schema versioning work outside this plan's scope). Exports the type so the desktop crate and CLI can reference it.

**Files:**
- Create: `crates/based-core/src/auth.rs`
- Modify: `crates/based-core/src/lib.rs` (add `pub mod auth; pub use auth::AuthMethod;`)

- [ ] **Step 1: Create `crates/based-core/src/auth.rs`**

```rust
// crates/based-core/src/auth.rs
//! Authentication method types.
//!
//! [`AuthMethod`] is the canonical representation of how Based authenticates
//! to a database. It is intentionally separate from engine-specific connection
//! configs so that auth concerns (SSH tunneling, IAM, vault) can evolve
//! without changing the engine config schemas.
//!
//! ## Migration path
//!
//! Today, auth is baked into each engine's config struct (e.g. `PostgresConfig`
//! has password/SSL fields). The eventual migration is:
//! 1. Add `auth: AuthMethod` to `ConnectionConfig` (with `default`)
//! 2. Move per-engine auth fields into `AuthMethod` variants
//! 3. Bump `.based/` `schema_version` and add a migration in `based-project`
//!
//! Until step 1 lands, `AuthMethod` is referenced by new code only (CLI flag
//! parsing, future connection wizard fields).

use serde::{Deserialize, Serialize};

/// How Based authenticates to a database endpoint.
///
/// Variants are additive — existing configs without an `auth` field deserialize
/// as [`AuthMethod::default()`] (password auth with an empty username, filled
/// in from `.env`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    /// Username + password. Password resolved from `.env` or secrets manager at connect time.
    Password {
        username: String,
    },
    /// SSH jump host wrapping another auth method (e.g. password to a private DB).
    SshTunnel {
        host: String,
        port: u16,
        username: String,
        /// Path to SSH private key; `None` uses the SSH agent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key_path: Option<String>,
        /// Auth method for the database endpoint behind the tunnel.
        inner: Box<AuthMethod>,
    },
    /// AWS IAM database authentication (RDS, DocumentDB, Atlas).
    AwsIam {
        region: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
    },
    /// Mutual TLS client certificate.
    ClientCertificate {
        cert_path: String,
        key_path: String,
    },
    /// Externally-managed token (OAuth2, OIDC, Vault). The `provider` string is
    /// an opaque identifier resolved by the credentials plugin at runtime.
    External {
        provider: String,
    },
}

impl Default for AuthMethod {
    /// Default to password auth; username resolved from `.env` at connect time.
    fn default() -> Self {
        Self::Password {
            username: String::new(),
        }
    }
}

impl AuthMethod {
    /// Returns `true` if this auth method requires a network hop before reaching
    /// the database (e.g. SSH tunnel). Used to annotate UI and telemetry.
    pub fn has_tunnel(&self) -> bool {
        matches!(self, Self::SshTunnel { .. })
    }

    /// Returns the innermost `AuthMethod` (unwraps SSH tunnels).
    pub fn inner_auth(&self) -> &AuthMethod {
        match self {
            Self::SshTunnel { inner, .. } => inner.inner_auth(),
            other => other,
        }
    }

    /// Human-readable label for UI display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Password { .. } => "Password",
            Self::SshTunnel { .. } => "SSH Tunnel",
            Self::AwsIam { .. } => "AWS IAM",
            Self::ClientCertificate { .. } => "Client Certificate",
            Self::External { .. } => "External / OAuth",
        }
    }
}
```

- [ ] **Step 2: Export from `based-core/src/lib.rs`**

Add:
```rust
pub mod auth;
pub use auth::AuthMethod;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check --workspace 2>&1 | head -40
```
Expected: no errors.

- [ ] **Step 4: Add a unit test in `auth.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_password() {
        assert!(matches!(AuthMethod::default(), AuthMethod::Password { .. }));
    }

    #[test]
    fn ssh_tunnel_has_tunnel() {
        let m = AuthMethod::SshTunnel {
            host: "bastion.example.com".into(),
            port: 22,
            username: "ec2-user".into(),
            key_path: None,
            inner: Box::new(AuthMethod::Password { username: "admin".into() }),
        };
        assert!(m.has_tunnel());
        assert!(!m.inner_auth().has_tunnel());
    }

    #[test]
    fn serde_round_trip_password() {
        let m = AuthMethod::Password { username: "alice".into() };
        let json = serde_json::to_string(&m).unwrap();
        let back: AuthMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn serde_round_trip_ssh_tunnel() {
        let m = AuthMethod::SshTunnel {
            host: "host".into(),
            port: 22,
            username: "u".into(),
            key_path: Some("/id_rsa".into()),
            inner: Box::new(AuthMethod::AwsIam {
                region: "us-east-1".into(),
                profile: None,
            }),
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: AuthMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p based-core 2>&1
```
Expected: 4 tests pass.

- [ ] **Step 6: Commit**

```bash
cd /Users/pavi2410/Projects/pavi2410/based
cargo fmt --all
git add crates/based-core/src/auth.rs crates/based-core/src/lib.rs
git commit -m "$(cat <<'EOF'
feat: add AuthMethod type to based-core (P4)

Introduces AuthMethod enum (Password, SshTunnel, AwsIam, ClientCertificate,
External) as a typed first-class auth abstraction in based-core. Includes
has_tunnel(), inner_auth(), and label() helpers.

Not yet wired into ConnectionConfig — migration path documented in auth.rs.
Four unit tests cover default, SSH tunnel, and serde round-trips.
EOF
)"
```

---

## Task 5: EditorContext service object (Priority 5)

**Goal:** Introduce an `EditorContext` GPUI entity that SQL editor panels create and hold. It carries the connection scope, a `VariableScope` (typed variables for autocomplete), and a `SchemaCache` (objects for LSP completion). Panels subscribe to it; future LSP and autocomplete features attach to it without touching panel internals.

**Files:**
- Create: `apps/desktop/src/editor/mod.rs`
- Create: `apps/desktop/src/editor/context.rs`
- Create: `apps/desktop/src/editor/variable_scope.rs`
- Create: `apps/desktop/src/editor/schema_cache.rs`
- Modify: `apps/desktop/src/main.rs` (add `mod editor;`)
- Modify: `apps/desktop/src/postgres/query_editor.rs` (add `editor_ctx` field, create in `new_with_initial`)
- Modify: `apps/desktop/src/sqlite/query_editor.rs` (same)

- [ ] **Step 1: Create `apps/desktop/src/editor/variable_scope.rs`**

```rust
// apps/desktop/src/editor/variable_scope.rs
//! Typed variable scope for editor autocomplete and query substitution.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// The data type of a scoped variable, used to drive autocomplete and type hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    /// Arbitrary JSON — type is unknown at the scope level.
    Json,
}

/// A single named variable available for substitution in the editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedVar {
    pub name: String,
    pub value: String,
    pub var_type: VarType,
    /// Human-readable description shown in autocomplete popover.
    pub description: Option<String>,
}

/// Collection of variables visible in a query editor.
///
/// Built from project-level `.env`/`vars.toml` plus any connection-level
/// overrides. Future: per-tab local variables.
#[derive(Debug, Clone, Default)]
pub struct VariableScope {
    vars: HashMap<String, ScopedVar>,
}

impl VariableScope {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a scope from a map of name → value (plain strings, as loaded from `.env`).
    pub fn from_string_map(map: &HashMap<String, String>) -> Self {
        let vars = map
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    ScopedVar {
                        name: k.clone(),
                        value: v.clone(),
                        var_type: VarType::String,
                        description: None,
                    },
                )
            })
            .collect();
        Self { vars }
    }

    pub fn get(&self, name: &str) -> Option<&ScopedVar> {
        self.vars.get(name)
    }

    pub fn all(&self) -> impl Iterator<Item = &ScopedVar> {
        self.vars.values()
    }

    pub fn names_sorted(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.vars.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}
```

- [ ] **Step 2: Create `apps/desktop/src/editor/schema_cache.rs`**

```rust
// apps/desktop/src/editor/schema_cache.rs
//! Cached schema objects for LSP completion and ER diagram data.

use std::sync::Arc;

use based_core::EngineKind;

/// A schema object (table, view, collection, etc.) visible to the editor.
#[derive(Debug, Clone)]
pub struct SchemaObject {
    /// Fully-qualified name (e.g. `public.users`, `mydb.orders`).
    pub full_name: String,
    /// Display label (unqualified, e.g. `users`).
    pub label: String,
    pub kind: ObjectKind,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectKind {
    Table,
    View,
    MaterializedView,
    Collection,
    Function,
    Procedure,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
}

/// Lazily-populated cache of schema objects for a single connection.
///
/// Panels request a population via [`SchemaCache::request_refresh`]; the
/// background task fills `objects` and notifies subscribers. LSP and
/// autocomplete consumers read from this cache without blocking the UI.
#[derive(Debug, Default)]
pub struct SchemaCache {
    pub engine: Option<EngineKind>,
    pub objects: Vec<SchemaObject>,
    pub last_refreshed_at: Option<std::time::Instant>,
}

impl SchemaCache {
    pub fn new(engine: EngineKind) -> Self {
        Self {
            engine: Some(engine),
            objects: vec![],
            last_refreshed_at: None,
        }
    }

    pub fn is_stale(&self) -> bool {
        self.last_refreshed_at
            .map(|t| t.elapsed().as_secs() > 300)
            .unwrap_or(true)
    }

    /// Fuzzy-search objects by prefix for autocomplete.
    pub fn complete(&self, prefix: &str) -> Vec<&SchemaObject> {
        let prefix_lower = prefix.to_lowercase();
        self.objects
            .iter()
            .filter(|o| o.label.to_lowercase().starts_with(&prefix_lower))
            .collect()
    }

    pub fn find_by_name(&self, name: &str) -> Option<&SchemaObject> {
        self.objects
            .iter()
            .find(|o| o.full_name == name || o.label == name)
    }
}
```

- [ ] **Step 3: Create `apps/desktop/src/editor/context.rs`**

```rust
// apps/desktop/src/editor/context.rs
//! Per-tab editor context — shared state for query editors.
//!
//! [`EditorContext`] is a GPUI entity held by each query editor panel.
//! It carries the connection scope, variable bindings, and a schema cache.
//! Future LSP clients attach here. Panels subscribe to changes for autocomplete.

use std::sync::Arc;

use based_core::EngineKind;
use gpui::EventEmitter;

use crate::connection::ConnectionId;

use super::{SchemaCache, VariableScope};

/// Events emitted when editor context state changes.
pub enum EditorContextEvent {
    VariablesChanged,
    SchemaCacheRefreshed,
}

/// Per-tab shared state for query editors.
///
/// Create one per editor panel. The panel owns the entity; other components
/// (autocomplete popup, explain overlay, lint runner) subscribe to it.
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
```

- [ ] **Step 4: Create `apps/desktop/src/editor/mod.rs`**

```rust
// apps/desktop/src/editor/mod.rs
//! Editor services — per-tab context, variable scoping, schema caching.
//!
//! These are engine-agnostic: all SQL editor panels (Postgres, SQLite) and
//! future LSP/autocomplete features share this layer.

pub mod context;
pub mod schema_cache;
pub mod variable_scope;

pub use context::{EditorContext, EditorContextEvent};
pub use schema_cache::{ColumnInfo, ObjectKind, SchemaCache, SchemaObject};
pub use variable_scope::{ScopedVar, VarType, VariableScope};
```

- [ ] **Step 5: Declare `mod editor` in `main.rs`**

Add `mod editor;` alongside the other module declarations (e.g. after `mod db;`).

- [ ] **Step 6: Add `editor_ctx` field to `postgres/query_editor.rs`**

Read `apps/desktop/src/postgres/query_editor.rs` first. Find the `QueryEditorPanel` struct definition and `new_with_initial` function.

Add the field to the struct:
```rust
use crate::editor::{EditorContext, VariableScope};
use gpui::Entity;

pub struct QueryEditorPanel {
    // ... existing fields ...
    pub editor_ctx: Entity<EditorContext>,
}
```

In `new_with_initial`, create the context:
```rust
pub fn new_with_initial(
    pool: PgPool,
    conn_id: ConnectionId,
    initial_sql: Option<String>,
    auto_run: bool,
    window: &mut Window,
    cx: &mut Context<Self>,
) -> Self {
    // Build variable scope from project vars global
    let variables = cx
        .try_global::<crate::project::ProjectVars>()
        .map(|pv| VariableScope::from_string_map(&pv.vars))
        .unwrap_or_default();

    let editor_ctx = cx.new(|_| {
        EditorContext::new(conn_id.clone(), based_core::EngineKind::Postgres, variables)
    });

    Self {
        // ... existing fields ...
        editor_ctx,
    }
}
```

- [ ] **Step 7: Add `editor_ctx` field to `sqlite/query_editor.rs`** (same pattern)

- [ ] **Step 8: Verify compilation**

```bash
cargo check -p desktop 2>&1 | head -60
```

- [ ] **Step 9: Run tests**

```bash
cargo test --workspace 2>&1 | tail -20
```

- [ ] **Step 10: Commit**

```bash
cd /Users/pavi2410/Projects/pavi2410/based
cargo fmt --all
git add apps/desktop/src/editor/ \
        apps/desktop/src/main.rs \
        apps/desktop/src/postgres/query_editor.rs \
        apps/desktop/src/sqlite/query_editor.rs
git commit -m "$(cat <<'EOF'
feat: add EditorContext service object for query editors (P5)

Introduces VariableScope, SchemaCache, and EditorContext types in the
new apps/desktop/src/editor/ module. Each SQL editor panel (Postgres,
SQLite) creates an EditorContext entity on construction.

EditorContext is the attachment point for future LSP clients, variable
autocomplete, schema-aware completion, and inline explain overlays —
none of those features require touching panel internals once wired here.
EOF
)"
```

---

## Task 6: TabScope and optional conn_id (Priority 6)

**Goal:** Add a `TabScope` enum to classify tabs by their connection scope (single connection, multi-connection, workspace, global). Change `TabSpec::conn_id()` to return `Option<&ConnectionId>` — removing the `HOME_CONN_SENTINEL` hack. Update all call sites.

**Files:**
- Modify: `apps/desktop/src/workspace/tabs/spec.rs` (add `TabScope`, add `scope()`, change `conn_id()` signature)
- Modify: `apps/desktop/src/workspace/tabs/manager.rs` (`close_tabs_for_conn` uses `conn_id()`)
- Modify: `apps/desktop/src/workspace/tabs/open.rs` (uses `conn_id()`)
- Modify: `apps/desktop/src/command_palette/selection.rs` (uses `conn_id()`)

- [ ] **Step 1: Find all `conn_id()` call sites**

```bash
grep -rn "\.conn_id()" apps/desktop/src/ --include="*.rs"
```

List every file:line. These are all sites that need updating.

- [ ] **Step 2: Add `TabScope` and `scope()` to `spec.rs`**

Add below the `HOME_CONN_SENTINEL`:
```rust
/// Classifies a tab by the number and kind of connections it operates on.
///
/// Use this to decide whether a tab should survive connection disconnect,
/// appear in connection-scoped menus, or be surfaced in workspace-level views
/// (e.g. ER diagrams, agent chat, charts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabScope {
    /// Tab operates on exactly one connection.
    Connection(ConnectionId),
    /// Tab spans multiple connections (ER diagrams, cross-DB joins).
    MultiConnection(Vec<ConnectionId>),
    /// Tab is scoped to the workspace but not a specific connection (agent chat, chart builder).
    Workspace,
    /// Tab is fully global — not tied to any project or connection (Home, ReleaseNotes, settings).
    Global,
}
```

Add `scope()` method to `TabSpec`:
```rust
impl TabSpec {
    pub fn scope(&self) -> TabScope {
        match self {
            Self::Home | Self::ReleaseNotes { .. } => TabScope::Global,
            Self::Dashboard(id) => TabScope::Connection(id.clone()),
            Self::DataViewer { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::QueryEditor { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::Pipeline { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::Inspector { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::ObjectInfo { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::DocumentInsert { conn_id, .. } => TabScope::Connection(conn_id.clone()),
            Self::Builtin { conn_id, .. } => conn_id
                .as_ref()
                .cloned()
                .map(TabScope::Connection)
                .unwrap_or(TabScope::Global),
        }
    }
}
```

- [ ] **Step 3: Change `conn_id()` to return `Option<&ConnectionId>`**

Replace the current `conn_id()` implementation:
```rust
pub fn conn_id(&self) -> Option<&ConnectionId> {
    match self {
        Self::Home | Self::ReleaseNotes { .. } => None,
        Self::Dashboard(id) => Some(id),
        Self::DataViewer { conn_id, .. } => Some(conn_id),
        Self::QueryEditor { conn_id, .. } => Some(conn_id),
        Self::Pipeline { conn_id, .. } => Some(conn_id),
        Self::Inspector { conn_id, .. } => Some(conn_id),
        Self::ObjectInfo { conn_id, .. } => Some(conn_id),
        Self::DocumentInsert { conn_id, .. } => Some(conn_id),
        Self::Builtin { conn_id, .. } => conn_id.as_ref(),
    }
}
```

Remove `HOME_CONN_SENTINEL` (no longer needed).

- [ ] **Step 4: Update `manager.rs::close_tabs_for_conn`**

Change:
```rust
// Old
.filter(|(_, t)| t.spec.conn_id() == conn_id)
// New
.filter(|(_, t)| t.spec.conn_id() == Some(conn_id))
```

- [ ] **Step 5: Update `dispatch.rs` — conn_id extraction**

In the new `dispatch_open_tab` (from Task 2), the conn_id is already extracted before the connection-independent tab check:
```rust
// Old:
let conn_id = spec.conn_id().clone();
// New:
let Some(conn_id) = spec.conn_id().cloned() else { return; };
```

(Connection-independent tabs already returned early before this line in Task 2's implementation.)

- [ ] **Step 6: Update all other call sites from Step 1**

For each site, change `spec.conn_id()` to `spec.conn_id().unwrap_or(...)` or `spec.conn_id().map(...)` as appropriate.

- [ ] **Step 7: Verify compilation**

```bash
cargo check -p desktop 2>&1 | head -60
```
Fix any remaining `Option<&ConnectionId>` vs `&ConnectionId` mismatches.

- [ ] **Step 8: Run tests**

```bash
cargo test --workspace 2>&1 | tail -20
```

- [ ] **Step 9: Commit**

```bash
cd /Users/pavi2410/Projects/pavi2410/based
cargo fmt --all
git add -A
git commit -m "$(cat <<'EOF'
refactor: add TabScope and fix conn_id() to return Option (P6)

Introduces TabScope enum (Connection | MultiConnection | Workspace | Global)
for classifying tabs by connection scope. Removes the HOME_CONN_SENTINEL
sentinel hack — TabSpec::conn_id() now cleanly returns Option<&ConnectionId>.

Workspace-scoped tabs (future: ER diagrams, agent chat, chart builder) and
global tabs (Home, ReleaseNotes) are now first-class, not sentinel-based.
EOF
)"
```

---

## Final verification

After all 6 tasks:

```bash
cd /Users/pavi2410/Projects/pavi2410/based
cargo check --workspace 2>&1 | grep "^error" | wc -l   # should be 0
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -20
git log --oneline -8
```

Expected: 0 errors, tests pass, 6 commits on `feat/architecture-extensibility`.
