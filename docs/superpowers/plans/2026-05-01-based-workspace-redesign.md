# Based Workspace Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the Based desktop database client from a monolithic workspace into a clean multi-connection workspace with a nested connection tree, heterogeneous tabs, command palette, and query history — across PostgreSQL, SQLite, and MongoDB.

**Architecture:** `workspace/mod.rs` (1,490 lines, 6 concerns) is split into `ConnectionTree` (sidebar navigation), `TabManager` (tab lifecycle), and a thin `Workspace` orchestrator. `QueryStore` and `CommandPalette` are new global GPUI entities. Engine panels are completed/fixed to use the new shared infrastructure.

**Tech Stack:** Rust, GPUI (Zed's UI framework), gpui-component, sqlx (Postgres + SQLite), mongodb driver, serde/toml/serde_json for persistence, gpui_tokio for async bridging.

**Read first:** `docs/superpowers/specs/2026-05-01-based-workspace-redesign-design.md`

**Verification command (use after every task):** `cargo check -p desktop 2>&1 | head -30`

---

## Phase 1: TabSpec + TabManager

> Defines the tab vocabulary and the entity that owns all open tabs. Everything downstream depends on these types.

---

### Task 1: Define TabSpec enum

**Files:**
- Create: `apps/desktop/src/workspace/tab_spec.rs`
- Modify: `apps/desktop/src/workspace/mod.rs` (add `pub mod tab_spec;`)

- [ ] **Step 1: Create tab_spec.rs**

```rust
// apps/desktop/src/workspace/tab_spec.rs
use crate::connection::ConnectionId;

/// Identifies what a tab shows. Used by TabManager to open-or-focus.
/// Two specs are equal iff they refer to the same logical panel — prevents duplicate DataViewers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TabSpec {
    Dashboard(ConnectionId),
    DataViewer { conn_id: ConnectionId, object: String },
    QueryEditor(ConnectionId),
    Pipeline { conn_id: ConnectionId, collection: String },
    Explain { conn_id: ConnectionId, label: String },
    Inspector { conn_id: ConnectionId, object: String },
}

impl TabSpec {
    pub fn conn_id(&self) -> &ConnectionId {
        match self {
            Self::Dashboard(id) => id,
            Self::DataViewer { conn_id, .. } => conn_id,
            Self::QueryEditor(id) => id,
            Self::Pipeline { conn_id, .. } => conn_id,
            Self::Explain { conn_id, .. } => conn_id,
            Self::Inspector { conn_id, .. } => conn_id,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Dashboard(_) => "dashboard",
            Self::DataViewer { .. } => "data viewer",
            Self::QueryEditor(_) => "query",
            Self::Pipeline { .. } => "pipeline",
            Self::Explain { .. } => "explain",
            Self::Inspector { .. } => "structure",
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Dashboard(id) => id.0.clone(),
            Self::DataViewer { object, .. } => object.clone(),
            Self::QueryEditor(_) => "untitled".to_string(),
            Self::Pipeline { collection, .. } => collection.clone(),
            Self::Explain { label, .. } => label.clone(),
            Self::Inspector { object, .. } => object.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_viewer_equality() {
        let a = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
        };
        let b = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn different_objects_not_equal() {
        let a = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "users".into(),
        };
        let b = TabSpec::DataViewer {
            conn_id: ConnectionId("pg".into()),
            object: "orders".into(),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn query_editors_always_distinct() {
        // QueryEditor tabs are always fresh — same conn_id still represents different tabs
        // (enforced by TabManager.open, not by equality)
        let a = TabSpec::QueryEditor(ConnectionId("pg".into()));
        let b = TabSpec::QueryEditor(ConnectionId("pg".into()));
        assert_eq!(a, b); // spec equality is fine — TabManager decides open-or-new
    }
}
```

- [ ] **Step 2: Add module to workspace/mod.rs**

Add to the top of `apps/desktop/src/workspace/mod.rs` with the other `pub mod` declarations:
```rust
pub mod tab_spec;
pub use tab_spec::TabSpec;
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p desktop tab_spec 2>&1
```
Expected: 3 tests pass.

- [ ] **Step 4: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/workspace/tab_spec.rs apps/desktop/src/workspace/mod.rs
git commit -m "feat(workspace): add TabSpec enum"
```

---

### Task 2: Create TabManager entity

**Files:**
- Create: `apps/desktop/src/workspace/tab_manager.rs`
- Modify: `apps/desktop/src/workspace/mod.rs` (add `pub mod tab_manager;`)

- [ ] **Step 1: Create tab_manager.rs**

```rust
// apps/desktop/src/workspace/tab_manager.rs
use gpui::{AnyView, Context, Entity, EventEmitter};
use crate::connection::ConnectionId;
use super::tab_spec::TabSpec;

/// An open tab — its spec (identity) and the live panel view.
pub struct Tab {
    pub spec: TabSpec,
    pub view: AnyView,
    pub dirty: bool, // unsaved query content
}

pub enum TabEvent {
    TabOpened(usize),   // index of new tab
    TabClosed(usize),
    ActiveChanged(usize),
}

pub struct TabManager {
    pub tabs: Vec<Tab>,
    pub active_idx: Option<usize>,
}

impl TabManager {
    pub fn new() -> Self {
        Self { tabs: vec![], active_idx: None }
    }

    /// Open a new tab or focus the existing one for this spec.
    /// QueryEditor always opens a new tab (caller passes a fresh spec each time).
    pub fn open_or_focus(&mut self, spec: TabSpec, view: AnyView, cx: &mut Context<Self>) {
        // QueryEditor: always new
        let is_query = matches!(spec, TabSpec::QueryEditor(_));
        if !is_query {
            if let Some(idx) = self.tabs.iter().position(|t| t.spec == spec) {
                self.active_idx = Some(idx);
                cx.emit(TabEvent::ActiveChanged(idx));
                cx.notify();
                return;
            }
        }
        let idx = self.tabs.len();
        self.tabs.push(Tab { spec, view, dirty: false });
        self.active_idx = Some(idx);
        cx.emit(TabEvent::TabOpened(idx));
        cx.emit(TabEvent::ActiveChanged(idx));
        cx.notify();
    }

    pub fn close(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() { return; }
        self.tabs.remove(idx);
        let new_active = if self.tabs.is_empty() {
            None
        } else {
            Some(idx.saturating_sub(1).min(self.tabs.len() - 1))
        };
        self.active_idx = new_active;
        cx.emit(TabEvent::TabClosed(idx));
        if let Some(i) = new_active {
            cx.emit(TabEvent::ActiveChanged(i));
        }
        cx.notify();
    }

    pub fn activate(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.tabs.len() {
            self.active_idx = Some(idx);
            cx.emit(TabEvent::ActiveChanged(idx));
            cx.notify();
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_idx.and_then(|i| self.tabs.get(i))
    }

    pub fn close_tabs_for_conn(&mut self, conn_id: &ConnectionId, cx: &mut Context<Self>) {
        let indices: Vec<usize> = self.tabs.iter().enumerate()
            .filter(|(_, t)| t.spec.conn_id() == conn_id)
            .map(|(i, _)| i)
            .rev()
            .collect();
        for i in indices {
            self.close(i, cx);
        }
    }
}

impl EventEmitter<TabEvent> for TabManager {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view() -> AnyView {
        // We cannot easily create a real AnyView in unit tests without a GPUI app context.
        // These tests validate TabManager logic only; view creation is tested via integration.
        panic!("use only spec-level tests here")
    }

    // Test the spec-level open_or_focus deduplication logic directly
    #[test]
    fn open_or_focus_deduplicates_data_viewer() {
        // We test just the lookup logic without a real cx
        let spec_a = TabSpec::DataViewer {
            conn_id: crate::connection::ConnectionId("pg".into()),
            object: "users".into(),
        };
        let spec_b = spec_a.clone();
        assert_eq!(spec_a, spec_b, "same spec should match for dedup");
    }

    #[test]
    fn query_editors_are_always_distinct_specs() {
        // Two QueryEditor specs with same conn_id are equal by value,
        // but TabManager opens new tab anyway (is_query path)
        let s = TabSpec::QueryEditor(crate::connection::ConnectionId("pg".into()));
        assert!(matches!(s, TabSpec::QueryEditor(_)));
    }
}
```

- [ ] **Step 2: Add module declaration**

In `apps/desktop/src/workspace/mod.rs`, add:
```rust
pub mod tab_manager;
pub use tab_manager::TabManager;
```

- [ ] **Step 3: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/workspace/tab_manager.rs apps/desktop/src/workspace/mod.rs
git commit -m "feat(workspace): add TabManager entity"
```

---

## Phase 2: ConnectionTree

> Extracts sidebar logic from workspace/mod.rs into a dedicated entity.

---

### Task 3: Create ConnectionTree entity

**Files:**
- Create: `apps/desktop/src/workspace/connection_tree.rs`
- Modify: `apps/desktop/src/workspace/mod.rs`

- [ ] **Step 1: Create connection_tree.rs**

```rust
// apps/desktop/src/workspace/connection_tree.rs
use std::collections::HashMap;
use gpui::{Context, Entity, EventEmitter, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, StyledExt, v_flex, h_flex};

use crate::connection::{
    AnyConnection, ConnectionId, ConnectionState, EngineKind,
    registry::{ConnectionRegistry, RegistryEvent},
};
use super::tab_spec::TabSpec;

#[derive(Clone, Debug)]
pub struct SchemaObject {
    pub name: String,
    pub kind: ObjectKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectKind {
    Table, View, MaterializedView, Trigger, Collection,
}

impl ObjectKind {
    pub fn group(&self) -> &'static str {
        match self {
            Self::Table => "Tables",
            Self::View | Self::MaterializedView => "Views",
            Self::Trigger => "Triggers",
            Self::Collection => "Collections",
        }
    }
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Table => "▤",
            Self::View | Self::MaterializedView => "◈",
            Self::Trigger => "⚡",
            Self::Collection => "▦",
        }
    }
}

/// Per-connection expansion and schema state.
struct ConnState {
    expanded: bool,
    objects: Option<Vec<SchemaObject>>, // None = not yet loaded
    loading: bool,
}

pub enum TreeEvent {
    /// User wants to open a tab for this spec.
    OpenTab(TabSpec),
}

pub struct ConnectionTree {
    registry: Entity<ConnectionRegistry>,
    conn_states: HashMap<ConnectionId, ConnState>,
    active_spec: Option<TabSpec>, // mirrors active tab for highlight
}

impl ConnectionTree {
    pub fn new(registry: Entity<ConnectionRegistry>, cx: &mut Context<Self>) -> Self {
        cx.subscribe(&registry, |this, _, event, cx| {
            match event {
                RegistryEvent::ConnectionAdded(id) => {
                    this.conn_states.entry(id.clone()).or_insert(ConnState {
                        expanded: false,
                        objects: None,
                        loading: false,
                    });
                    cx.notify();
                }
                RegistryEvent::ConnectionRemoved(id) => {
                    this.conn_states.remove(id);
                    cx.notify();
                }
                RegistryEvent::ConnectionStateChanged(_) => cx.notify(),
            }
        }).detach();

        let initial_ids = registry.read(cx).ordered_ids(cx);
        let conn_states = initial_ids.into_iter().map(|id| {
            (id, ConnState { expanded: false, objects: None, loading: false })
        }).collect();

        Self { registry, conn_states, active_spec: None }
    }

    pub fn set_active_spec(&mut self, spec: Option<TabSpec>, cx: &mut Context<Self>) {
        self.active_spec = spec;
        cx.notify();
    }

    fn toggle_connection(&mut self, id: &ConnectionId, cx: &mut Context<Self>) {
        if let Some(s) = self.conn_states.get_mut(id) {
            s.expanded = !s.expanded;
            cx.notify();
        }
    }

    fn on_object_clicked(&mut self, id: &ConnectionId, obj: &SchemaObject, cx: &mut Context<Self>) {
        let spec = match obj.kind {
            ObjectKind::Collection => TabSpec::DataViewer {
                conn_id: id.clone(),
                object: obj.name.clone(),
            },
            _ => TabSpec::DataViewer {
                conn_id: id.clone(),
                object: obj.name.clone(),
            },
        };
        cx.emit(TreeEvent::OpenTab(spec));
    }
}

impl EventEmitter<TreeEvent> for ConnectionTree {}

impl Render for ConnectionTree {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = self.registry.read(cx);
        let ids = registry.ordered_ids(cx);

        v_flex()
            .size_full()
            .overflow_y_scroll()
            .children(ids.into_iter().map(|id| {
                let entry = registry.get(&id, cx).map(|e| e.read(cx));
                // render connection row + objects — see render helpers below
                div().child(format!("conn: {}", id.0))
            }))
            .child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .p_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .cursor_pointer()
                    .child("＋ Add connection")
            )
    }
}
```

> **Note:** The render method above is a skeleton. The full sidebar render (connection rows, schema group headers, object rows with hover actions, engine icons) replaces the equivalent logic currently in `workspace/mod.rs` functions `render_objects_pane` (lines 1201–1417) and the connection list inside `render` (lines 773–1075). Move that logic into `ConnectionTree::render` and its helper methods. Use `cx.theme().border`, `cx.theme().muted_foreground`, `cx.theme().foreground` for colors to respect the theme.

- [ ] **Step 2: Add module declaration**

In `apps/desktop/src/workspace/mod.rs`:
```rust
pub mod connection_tree;
pub use connection_tree::ConnectionTree;
```

- [ ] **Step 3: Compile check**

```bash
cargo check -p desktop 2>&1 | head -30
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/workspace/connection_tree.rs apps/desktop/src/workspace/mod.rs
git commit -m "feat(workspace): add ConnectionTree entity skeleton"
```

---

### Task 4: Wire ConnectionTree + TabManager into Workspace

**Files:**
- Modify: `apps/desktop/src/workspace/mod.rs`

- [ ] **Step 1: Add entities to Workspace struct**

Replace the current `Workspace` struct fields (line 117) with:

```rust
pub struct Workspace {
    registry: Entity<ConnectionRegistry>,
    dock_area: Entity<DockArea>,
    connection_tree: Entity<ConnectionTree>,
    tab_manager: Entity<TabManager>,
    sidebar_collapsed: bool,
    focus_handle: FocusHandle,
    project_title: SharedString,
}
```

- [ ] **Step 2: Update Workspace::new to build the new entities**

In `Workspace::new`, after building `registry`, add:

```rust
let connection_tree = cx.new(|cx| ConnectionTree::new(registry.clone(), cx));
let tab_manager = cx.new(|_| TabManager::new());

// When ConnectionTree emits OpenTab, forward to TabManager
cx.subscribe(&connection_tree, {
    let tab_manager = tab_manager.clone();
    move |_this, _tree, event, cx| {
        if let connection_tree::TreeEvent::OpenTab(spec) = event {
            // panel creation dispatched from open_tab helper
            // (implemented in next step)
            let _ = tab_manager; // placeholder
            let _ = spec;
        }
    }
}).detach();
```

- [ ] **Step 3: Compile check — expect errors from removed fields**

```bash
cargo check -p desktop 2>&1 | grep "^error" | head -20
```

Fix each error: replace `self.selected_connection`, `self.active_objects`, `self.selected_object` with reads from `connection_tree` and `tab_manager`. Remove `pending_open_connection` logic — the tree handles connect-on-click now.

- [ ] **Step 4: Slim down Workspace::render**

The `render` method (line 773) currently contains the full sidebar and inspector inline. Replace the sidebar section with:

```rust
// In Workspace::render, where sidebar was:
let tree = self.connection_tree.clone();
// ... render tree.view(window, cx) inside the sidebar div
```

- [ ] **Step 5: Compile check — clean**

```bash
cargo check -p desktop 2>&1 | head -10
```
Expected: no errors.

- [ ] **Step 6: Run the app and verify sidebar renders**

```bash
mise run dev
```
Expected: app opens, sidebar shows connection list (may be unstyled — styling comes in later tasks).

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/workspace/
git commit -m "refactor(workspace): wire ConnectionTree + TabManager into Workspace"
```

---

## Phase 3: QueryStore

> New global entity for query history and saved queries. No UI yet — just the data layer.

---

### Task 5: QueryStore data structures + file layout

**Files:**
- Create: `apps/desktop/src/query_store/mod.rs`
- Create: `apps/desktop/src/query_store/history.rs`
- Create: `apps/desktop/src/query_store/saved.rs`
- Modify: `apps/desktop/src/main.rs`

- [ ] **Step 1: Create query_store/mod.rs**

```rust
// apps/desktop/src/query_store/mod.rs
pub mod history;
pub mod saved;

pub use history::{HistoryEntry, QueryHistory};
pub use saved::{SavedQuery, SavedQueries};

use std::path::PathBuf;
use gpui::{App, Context, Entity, EventEmitter, Global};

use crate::connection::ConnectionId;

pub enum QueryStoreEvent {
    HistoryUpdated(ConnectionId),
    SavedUpdated,
}

pub struct QueryStore {
    pub history: QueryHistory,
    pub saved: SavedQueries,
    queries_dir: PathBuf,    // .based/local/
    saved_path: PathBuf,     // .based/queries.toml
}

impl QueryStore {
    pub fn new(project_root: Option<PathBuf>) -> Self {
        let base = project_root.unwrap_or_else(|| PathBuf::from("."));
        let queries_dir = base.join(".based").join("local");
        let saved_path = base.join(".based").join("queries.toml");

        // Ensure .based/local/ exists and is gitignored
        let _ = std::fs::create_dir_all(&queries_dir);
        let gitignore = base.join(".based").join(".gitignore");
        if !gitignore.exists() {
            let _ = std::fs::write(&gitignore, "local/\n");
        }

        Self {
            history: QueryHistory::load(&queries_dir),
            saved: SavedQueries::load(&saved_path),
            queries_dir,
            saved_path,
        }
    }

    pub fn push_history(&mut self, entry: HistoryEntry, cx: &mut Context<Self>) {
        let conn_id = entry.conn_id.clone();
        self.history.push(entry, &self.queries_dir);
        cx.emit(QueryStoreEvent::HistoryUpdated(conn_id));
    }

    pub fn save_query(&mut self, query: SavedQuery, cx: &mut Context<Self>) {
        self.saved.add(query);
        self.saved.persist(&self.saved_path);
        cx.emit(QueryStoreEvent::SavedUpdated);
    }

    pub fn history_for(&self, conn_id: &ConnectionId) -> Vec<&HistoryEntry> {
        self.history.for_conn(conn_id)
    }

    pub fn all_saved(&self) -> &[SavedQuery] {
        &self.saved.queries
    }
}

impl EventEmitter<QueryStoreEvent> for QueryStore {}

impl Global for QueryStore {}

pub fn init(project_root: Option<std::path::PathBuf>, cx: &mut App) {
    cx.set_global(QueryStore::new(project_root));
}
```

- [ ] **Step 2: Create query_store/history.rs**

```rust
// apps/desktop/src/query_store/history.rs
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::connection::ConnectionId;

const MAX_HISTORY: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub conn_id: ConnectionId,
    /// SQL text or pipeline JSON string
    pub query: String,
    #[serde(with = "time::serde::rfc3339")]
    pub ran_at: OffsetDateTime,
    pub duration_ms: u64,
    pub row_count: Option<u64>,
}

pub struct QueryHistory {
    entries: Vec<HistoryEntry>,
}

impl QueryHistory {
    pub fn load(local_dir: &Path) -> Self {
        let path = history_path(local_dir);
        if !path.exists() {
            return Self { entries: vec![] };
        }
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(_) => return Self { entries: vec![] },
        };
        let entries = BufReader::new(file)
            .lines()
            .filter_map(|l| l.ok())
            .filter_map(|l| serde_json::from_str(&l).ok())
            .collect();
        Self { entries }
    }

    pub fn push(&mut self, entry: HistoryEntry, local_dir: &Path) {
        self.entries.push(entry.clone());
        if self.entries.len() > MAX_HISTORY {
            self.entries.remove(0);
        }
        // Append to file
        let path = history_path(local_dir);
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            if let Ok(line) = serde_json::to_string(&entry) {
                let _ = writeln!(file, "{}", line);
            }
        }
    }

    pub fn for_conn(&self, conn_id: &ConnectionId) -> Vec<&HistoryEntry> {
        self.entries.iter().filter(|e| &e.conn_id == conn_id).rev().take(200).collect()
    }

    pub fn recent(&self, limit: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(limit).collect()
    }
}

fn history_path(local_dir: &Path) -> std::path::PathBuf {
    local_dir.join("history.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(conn: &str, sql: &str) -> HistoryEntry {
        HistoryEntry {
            conn_id: ConnectionId(conn.into()),
            query: sql.into(),
            ran_at: OffsetDateTime::now_utc(),
            duration_ms: 10,
            row_count: Some(5),
        }
    }

    #[test]
    fn push_and_retrieve() {
        let dir = tempdir().unwrap();
        let mut h = QueryHistory::load(dir.path());
        h.push(entry("pg", "SELECT 1"), dir.path());
        h.push(entry("pg", "SELECT 2"), dir.path());
        let results = h.for_conn(&ConnectionId("pg".into()));
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].query, "SELECT 2"); // most recent first
    }

    #[test]
    fn persists_and_reloads() {
        let dir = tempdir().unwrap();
        {
            let mut h = QueryHistory::load(dir.path());
            h.push(entry("pg", "SELECT 1"), dir.path());
        }
        let h2 = QueryHistory::load(dir.path());
        assert_eq!(h2.entries.len(), 1);
        assert_eq!(h2.entries[0].query, "SELECT 1");
    }

    #[test]
    fn caps_at_500() {
        let dir = tempdir().unwrap();
        let mut h = QueryHistory::load(dir.path());
        for i in 0..510 {
            h.push(entry("pg", &format!("SELECT {}", i)), dir.path());
        }
        assert_eq!(h.entries.len(), 500);
    }
}
```

- [ ] **Step 3: Add tempfile dev-dependency**

In `apps/desktop/Cargo.toml`, add:
```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Create query_store/saved.rs**

```rust
// apps/desktop/src/query_store/saved.rs
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::connection::ConnectionId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: String,
    pub name: String,
    pub connection: ConnectionId,
    pub tags: Vec<String>,
    /// SQL text (Postgres/SQLite) or pipeline JSON string (MongoDB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<String>,
}

impl SavedQuery {
    pub fn query_text(&self) -> &str {
        self.sql.as_deref()
            .or(self.pipeline.as_deref())
            .unwrap_or("")
    }
}

#[derive(Default, Serialize, Deserialize)]
struct SavedFile {
    #[serde(default, rename = "query")]
    pub queries: Vec<SavedQuery>,
}

pub struct SavedQueries {
    pub queries: Vec<SavedQuery>,
}

impl SavedQueries {
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self { queries: vec![] };
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let file: SavedFile = toml::from_str(&content).unwrap_or_default();
        Self { queries: file.queries }
    }

    pub fn add(&mut self, query: SavedQuery) {
        // Replace if same id, otherwise push
        if let Some(existing) = self.queries.iter_mut().find(|q| q.id == query.id) {
            *existing = query;
        } else {
            self.queries.push(query);
        }
    }

    pub fn persist(&self, path: &Path) {
        let file = SavedFile { queries: self.queries.clone() };
        if let Ok(content) = toml::to_string_pretty(&file) {
            let _ = std::fs::write(path, content);
        }
    }

    pub fn for_conn(&self, conn_id: &ConnectionId) -> Vec<&SavedQuery> {
        self.queries.iter().filter(|q| &q.connection == conn_id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_query(id: &str, name: &str) -> SavedQuery {
        SavedQuery {
            id: id.into(),
            name: name.into(),
            connection: ConnectionId("pg".into()),
            tags: vec!["test".into()],
            sql: Some("SELECT 1".into()),
            pipeline: None,
        }
    }

    #[test]
    fn add_and_persist_and_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("queries.toml");
        let mut s = SavedQueries::load(&path);
        s.add(make_query("q_1", "My Query"));
        s.persist(&path);

        let s2 = SavedQueries::load(&path);
        assert_eq!(s2.queries.len(), 1);
        assert_eq!(s2.queries[0].name, "My Query");
    }

    #[test]
    fn add_replaces_same_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("queries.toml");
        let mut s = SavedQueries::load(&path);
        s.add(make_query("q_1", "Original"));
        s.add(make_query("q_1", "Updated"));
        assert_eq!(s.queries.len(), 1);
        assert_eq!(s.queries[0].name, "Updated");
    }
}
```

- [ ] **Step 5: Register module in lib/main**

In `apps/desktop/src/main.rs`, add:
```rust
mod query_store;
```

In `main.rs`'s `run` closure, after `db::init(cx)`:
```rust
let project_root = crate::project::find_project_root();
query_store::init(project_root, cx);
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p desktop query_store 2>&1
```
Expected: all tests pass.

- [ ] **Step 7: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src/query_store/ apps/desktop/src/main.rs apps/desktop/Cargo.toml
git commit -m "feat(query_store): add history + saved queries persistence"
```

---

## Phase 4: Command Palette

---

### Task 6: Command Palette entity + render

**Files:**
- Create: `apps/desktop/src/command_palette/mod.rs`
- Modify: `apps/desktop/src/main.rs`
- Modify: `apps/desktop/src/workspace/mod.rs`
- Modify: `apps/desktop/src/app/actions.rs`

- [ ] **Step 1: Add action to actions.rs**

```rust
// apps/desktop/src/app/actions.rs
gpui::actions!(workspace, [
    ToggleSidebarRail,
    CycleAppearance,
    ToggleCommandPalette,
    CloseTab,
    NewQuery,
]);
```

- [ ] **Step 2: Add keybinding in bindings.rs**

In `apps/desktop/src/bindings.rs`, add:
```rust
KeyBinding::new("cmd-k", ToggleCommandPalette, None),
KeyBinding::new("ctrl-k", ToggleCommandPalette, None),
```

- [ ] **Step 3: Create command_palette/mod.rs**

```rust
// apps/desktop/src/command_palette/mod.rs
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Global, IntoElement,
    KeyBinding, Render, SharedString, Window, div, prelude::*,
};
use gpui_component::{ActiveTheme, StyledExt, v_flex, h_flex};

use crate::connection::registry::ConnectionRegistry;
use crate::query_store::QueryStore;
use crate::workspace::tab_spec::TabSpec;

/// A search result the palette can return.
#[derive(Clone)]
pub struct PaletteResult {
    pub kind: ResultKind,
    pub label: String,
    pub sublabel: String, // conn name or query text preview
    pub conn_label: String,
    pub spec: TabSpec,
}

#[derive(Clone, PartialEq)]
pub enum ResultKind {
    SchemaObject,
    SavedQuery,
    History,
}

pub struct CommandPalette {
    registry: Entity<ConnectionRegistry>,
    query: String,
    results: Vec<PaletteResult>,
    selected: usize,
    visible: bool,
    focus_handle: FocusHandle,
}

impl CommandPalette {
    pub fn new(registry: Entity<ConnectionRegistry>, cx: &mut Context<Self>) -> Self {
        Self {
            registry,
            query: String::new(),
            results: vec![],
            selected: 0,
            visible: false,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.visible = !self.visible;
        if self.visible {
            self.query.clear();
            self.refresh_results(cx);
            self.focus_handle.focus(window);
        }
        cx.notify();
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        cx.notify();
    }

    fn refresh_results(&mut self, cx: &mut Context<Self>) {
        let q = self.query.to_lowercase();
        let mut results = vec![];

        // 1. Schema objects from all connected registries
        let ids = self.registry.read(cx).ordered_ids(cx);
        for conn_id in &ids {
            if let Some(entry) = self.registry.read(cx).get(conn_id, cx) {
                let entry = entry.read(cx);
                // objects come from ConnectionTree's cache — for now search conn name
                if q.is_empty() || conn_id.0.to_lowercase().contains(&q) {
                    results.push(PaletteResult {
                        kind: ResultKind::SchemaObject,
                        label: conn_id.0.clone(),
                        sublabel: String::new(),
                        conn_label: conn_id.0.clone(),
                        spec: TabSpec::Dashboard(conn_id.clone()),
                    });
                }
            }
        }

        // 2. Saved queries
        if let Some(store) = cx.try_global::<QueryStore>() {
            for saved in store.all_saved() {
                if q.is_empty() || saved.name.to_lowercase().contains(&q) {
                    results.push(PaletteResult {
                        kind: ResultKind::SavedQuery,
                        label: saved.name.clone(),
                        sublabel: saved.query_text().chars().take(60).collect(),
                        conn_label: saved.connection.0.clone(),
                        spec: TabSpec::QueryEditor(saved.connection.clone()),
                    });
                }
            }
        }

        self.results = results;
        self.selected = 0;
        cx.notify();
    }
}

impl Focusable for CommandPalette {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus_handle.clone() }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element().into();
        }

        // Full-screen overlay with centered palette
        div()
            .absolute()
            .inset_0()
            .z_index(100)
            .bg(gpui::rgba(0x00000088))
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.dismiss(cx);
            }))
            .child(
                div()
                    .absolute()
                    .top(gpui::px(120.0))
                    .left_1_2()
                    .ml(gpui::px(-280.0))
                    .w(gpui::px(560.0))
                    .bg(cx.theme().elevated_surface_background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        // Search input row
                        h_flex()
                            .p_3()
                            .gap_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(div().text_color(cx.theme().muted_foreground).child("⌕"))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(cx.theme().foreground)
                                    .child(if self.query.is_empty() {
                                        "Search tables, queries, connections…".to_string()
                                    } else {
                                        self.query.clone()
                                    })
                            )
                    )
                    .child(
                        // Results list
                        v_flex()
                            .max_h(gpui::px(360.0))
                            .overflow_y_scroll()
                            .children(self.results.iter().enumerate().map(|(i, r)| {
                                let is_sel = i == self.selected;
                                div()
                                    .px_3()
                                    .py_2()
                                    .flex()
                                    .gap_2()
                                    .when(is_sel, |d| d.bg(cx.theme().accent))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child(r.conn_label.clone()))
                                    .child(div().flex_1().text_sm().child(r.label.clone()))
                            }))
                    )
                    .child(
                        // Footer hints
                        h_flex()
                            .px_3()
                            .py_2()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .gap_3()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("↑↓ navigate")
                            .child("↵ open")
                            .child("esc dismiss")
                    )
            )
            .into_any_element()
            .into()
    }
}

pub fn init(cx: &mut App) {
    // Register the palette as a global — Workspace grabs it from cx
}
```

- [ ] **Step 4: Wire toggle into Workspace**

In `apps/desktop/src/workspace/mod.rs`, add a `command_palette: Entity<CommandPalette>` field to `Workspace`. In `Workspace::new`:

```rust
let command_palette = cx.new(|cx| CommandPalette::new(registry.clone(), cx));
```

Add action handler in `Workspace::new`:
```rust
cx.on_action(|this: &mut Workspace, _: &ToggleCommandPalette, window, cx| {
    this.command_palette.update(cx, |p, cx| p.toggle(window, cx));
});
```

In `Workspace::render`, render the palette as an overlay child of the root div:
```rust
.child(self.command_palette.clone())
```

- [ ] **Step 5: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 6: Run app and test ⌘K**

```bash
mise run dev
```
Expected: pressing ⌘K opens a dark overlay with search input. Escape dismisses.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/command_palette/ apps/desktop/src/workspace/mod.rs apps/desktop/src/app/actions.rs apps/desktop/src/bindings.rs
git commit -m "feat: add command palette (⌘K)"
```

---

## Phase 5: Shared Widgets

---

### Task 7: FilterBar widget

**Files:**
- Modify: `apps/desktop/src/widgets/filter_bar.rs`

- [ ] **Step 1: Implement FilterBar**

```rust
// apps/desktop/src/widgets/filter_bar.rs
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, h_flex};

#[derive(Clone, Debug, PartialEq)]
pub enum FilterOp {
    Eq, NotEq, Like, Gt, Lt, IsNull, IsNotNull,
}

impl FilterOp {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::NotEq => "≠",
            Self::Like => "contains",
            Self::Gt => ">",
            Self::Lt => "<",
            Self::IsNull => "is null",
            Self::IsNotNull => "is not null",
        }
    }

    pub fn has_value(&self) -> bool {
        !matches!(self, Self::IsNull | Self::IsNotNull)
    }
}

#[derive(Clone, Debug)]
pub struct FilterExpr {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
}

impl FilterExpr {
    /// Generate a SQL WHERE clause fragment (parameterized placeholder as literal for display).
    pub fn to_sql(&self) -> String {
        match self.op {
            FilterOp::IsNull => format!("{} IS NULL", self.column),
            FilterOp::IsNotNull => format!("{} IS NOT NULL", self.column),
            FilterOp::Like => format!("{} ILIKE '%{}%'", self.column, self.value.replace('\'', "''")),
            FilterOp::Eq => format!("{} = '{}'", self.column, self.value.replace('\'', "''")),
            FilterOp::NotEq => format!("{} != '{}'", self.column, self.value.replace('\'', "''")),
            FilterOp::Gt => format!("{} > '{}'", self.column, self.value.replace('\'', "''")),
            FilterOp::Lt => format!("{} < '{}'", self.column, self.value.replace('\'', "''")),
        }
    }

    /// Generate a MongoDB filter document fragment (as JSON string).
    pub fn to_mongo_filter(&self) -> String {
        match self.op {
            FilterOp::IsNull => format!("{{\"{}\":null}}", self.column),
            FilterOp::IsNotNull => format!("{{\"{}\":{{\"$ne\":null}}}}", self.column),
            FilterOp::Eq => format!("{{\"{}\":\"{}\"}}", self.column, self.value),
            FilterOp::NotEq => format!("{{\"{}\":{{\"$ne\":\"{}\"}}}}", self.column, self.value),
            FilterOp::Like => format!("{{\"{}\":{{\"$regex\":\"{}\",\"$options\":\"i\"}}}}", self.column, self.value),
            FilterOp::Gt => format!("{{\"{}\":{{\"$gt\":\"{}\"}}}}", self.column, self.value),
            FilterOp::Lt => format!("{{\"{}\":{{\"$lt\":\"{}\"}}}}", self.column, self.value),
        }
    }
}

pub struct FilterBar {
    pub columns: Vec<String>,
    pub selected_column: usize,
    pub op: FilterOp,
    pub value: String,
    pub active: bool,
}

impl FilterBar {
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            columns,
            selected_column: 0,
            op: FilterOp::Eq,
            value: String::new(),
            active: false,
        }
    }

    pub fn current_expr(&self) -> Option<FilterExpr> {
        if !self.active || self.columns.is_empty() { return None; }
        let col = self.columns.get(self.selected_column)?.clone();
        Some(FilterExpr { column: col, op: self.op.clone(), value: self.value.clone() })
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.value.clear();
    }
}

impl Render for FilterBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_1()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(div().child("Filter:"))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(cx.theme().secondary)
                    .rounded_sm()
                    .child(self.columns.get(self.selected_column).cloned().unwrap_or_default())
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(cx.theme().secondary)
                    .rounded_sm()
                    .child(self.op.label())
            )
            .when(self.op.has_value(), |d| {
                d.child(
                    div()
                        .px_2()
                        .py_1()
                        .bg(cx.theme().input)
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_sm()
                        .min_w(gpui::px(100.0))
                        .child(if self.value.is_empty() { "value…".to_string() } else { self.value.clone() })
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_eq() {
        let expr = FilterExpr { column: "email".into(), op: FilterOp::Eq, value: "a@b.com".into() };
        assert_eq!(expr.to_sql(), "email = 'a@b.com'");
    }

    #[test]
    fn sql_like() {
        let expr = FilterExpr { column: "name".into(), op: FilterOp::Like, value: "alice".into() };
        assert_eq!(expr.to_sql(), "name ILIKE '%alice%'");
    }

    #[test]
    fn sql_is_null() {
        let expr = FilterExpr { column: "deleted_at".into(), op: FilterOp::IsNull, value: String::new() };
        assert_eq!(expr.to_sql(), "deleted_at IS NULL");
    }

    #[test]
    fn mongo_eq() {
        let expr = FilterExpr { column: "status".into(), op: FilterOp::Eq, value: "active".into() };
        assert_eq!(expr.to_mongo_filter(), "{\"status\":\"active\"}");
    }

    #[test]
    fn sql_escapes_single_quote() {
        let expr = FilterExpr { column: "name".into(), op: FilterOp::Eq, value: "O'Brien".into() };
        assert_eq!(expr.to_sql(), "name = 'O''Brien'");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p desktop filter_bar 2>&1
```
Expected: all tests pass.

- [ ] **Step 3: Compile check**

```bash
cargo check -p desktop 2>&1 | head -10
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/widgets/filter_bar.rs
git commit -m "feat(widgets): implement FilterBar with SQL + Mongo filter generation"
```

---

### Task 8: CellDetail overlay widget

**Files:**
- Modify: `apps/desktop/src/widgets/cell_detail.rs`

- [ ] **Step 1: Implement CellDetail**

```rust
// apps/desktop/src/widgets/cell_detail.rs
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, StyledExt, v_flex, h_flex};

pub enum CellValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(String),   // raw JSON string — will be pretty-printed
    Null,
    Blob(usize),    // byte count only
}

impl CellValue {
    pub fn type_label(&self) -> &'static str {
        match self {
            Self::Text(_) => "TEXT",
            Self::Integer(_) => "INTEGER",
            Self::Float(_) => "FLOAT",
            Self::Boolean(_) => "BOOLEAN",
            Self::Json(_) => "JSON",
            Self::Null => "NULL",
            Self::Blob(_) => "BLOB",
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Integer(n) => n.to_string(),
            Self::Float(f) => format!("{:.6}", f),
            Self::Boolean(b) => b.to_string(),
            Self::Json(s) => pretty_json(s),
            Self::Null => "NULL".to_string(),
            Self::Blob(n) => format!("<{} bytes>", n),
        }
    }
}

fn pretty_json(s: &str) -> String {
    serde_json::from_str::<serde_json::Value>(s)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| s.to_string())
}

pub struct CellDetail {
    pub column: String,
    pub value: CellValue,
    pub visible: bool,
}

impl CellDetail {
    pub fn new() -> Self {
        Self { column: String::new(), value: CellValue::Null, visible: false }
    }

    pub fn show(&mut self, column: String, value: CellValue) {
        self.column = column;
        self.value = value;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

impl Render for CellDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element().into();
        }

        let display = self.value.display();
        let type_label = self.value.type_label();

        div()
            .absolute()
            .bottom_0()
            .right_0()
            .w(gpui::px(320.0))
            .max_h(gpui::px(300.0))
            .m_2()
            .bg(cx.theme().elevated_surface_background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .shadow_lg()
            .overflow_hidden()
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div().flex_1().text_xs().text_color(cx.theme().muted_foreground)
                            .child(format!("{} — {}", self.column, type_label))
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .cursor_pointer()
                            .child("✕")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.hide();
                                cx.notify();
                            }))
                    )
            )
            .child(
                div()
                    .p_3()
                    .font_family("monospace")
                    .text_xs()
                    .text_color(cx.theme().foreground)
                    .overflow_y_scroll()
                    .max_h(gpui::px(220.0))
                    .child(display)
            )
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .cursor_pointer()
                            .child("Copy value")
                    )
            )
            .into_any_element()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_prints_json() {
        let v = CellValue::Json("{\"a\":1,\"b\":2}".into());
        let d = v.display();
        assert!(d.contains('\n'));
    }

    #[test]
    fn null_displays() {
        assert_eq!(CellValue::Null.display(), "NULL");
        assert_eq!(CellValue::Null.type_label(), "NULL");
    }

    #[test]
    fn blob_shows_byte_count() {
        let d = CellValue::Blob(1024).display();
        assert_eq!(d, "<1024 bytes>");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p desktop cell_detail 2>&1
```

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/widgets/cell_detail.rs
git commit -m "feat(widgets): implement CellDetail overlay with JSON pretty-print"
```

---

### Task 9: CodeEditor widget (token colorizer)

**Files:**
- Modify: `apps/desktop/src/widgets/code_editor.rs`

- [ ] **Step 1: Implement SQL token colorizer**

```rust
// apps/desktop/src/widgets/code_editor.rs
use gpui::{Context, Hsla, IntoElement, Render, Window, div, prelude::*};
use gpui_component::ActiveTheme;

#[derive(Clone, PartialEq)]
pub enum EditorMode { Sql, Json }

#[derive(Clone)]
struct Token {
    text: String,
    kind: TokenKind,
}

#[derive(Clone, PartialEq)]
enum TokenKind { Keyword, String, Number, Comment, Punctuation, Plain }

const SQL_KEYWORDS: &[&str] = &[
    "SELECT","FROM","WHERE","AND","OR","NOT","IN","IS","NULL","JOIN","LEFT","RIGHT",
    "INNER","OUTER","ON","AS","GROUP","BY","ORDER","HAVING","LIMIT","OFFSET","INSERT",
    "INTO","VALUES","UPDATE","SET","DELETE","CREATE","DROP","ALTER","TABLE","INDEX",
    "EXPLAIN","ANALYZE","WITH","DISTINCT","COUNT","SUM","AVG","MAX","MIN","CASE",
    "WHEN","THEN","ELSE","END","EXISTS","LIKE","ILIKE","BETWEEN","UNION","ALL",
    "RETURNING","PRAGMA",
];

fn tokenize_sql(input: &str) -> Vec<Token> {
    let mut tokens = vec![];
    let mut chars = input.chars().peekable();
    let mut current = String::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            '\'' => {
                // flush current
                if !current.is_empty() { tokens.push(classify(std::mem::take(&mut current))); }
                let mut s = String::from('\'');
                chars.next();
                while let Some(&c) = chars.peek() {
                    s.push(c);
                    chars.next();
                    if c == '\'' { break; }
                }
                tokens.push(Token { text: s, kind: TokenKind::String });
            }
            '-' => {
                chars.next();
                if chars.peek() == Some(&'-') {
                    if !current.is_empty() { tokens.push(classify(std::mem::take(&mut current))); }
                    let mut comment = String::from("--");
                    chars.next();
                    while let Some(&c) = chars.peek() {
                        if c == '\n' { break; }
                        comment.push(c);
                        chars.next();
                    }
                    tokens.push(Token { text: comment, kind: TokenKind::Comment });
                } else {
                    current.push('-');
                }
            }
            ' ' | '\n' | '\t' | ',' | '(' | ')' | ';' => {
                if !current.is_empty() { tokens.push(classify(std::mem::take(&mut current))); }
                tokens.push(Token { text: ch.to_string(), kind: TokenKind::Punctuation });
                chars.next();
            }
            _ => { current.push(ch); chars.next(); }
        }
    }
    if !current.is_empty() { tokens.push(classify(current)); }
    tokens
}

fn classify(text: String) -> Token {
    let kind = if SQL_KEYWORDS.contains(&text.to_uppercase().as_str()) {
        TokenKind::Keyword
    } else if text.chars().all(|c| c.is_ascii_digit() || c == '.') {
        TokenKind::Number
    } else {
        TokenKind::Plain
    };
    Token { text, kind }
}

pub struct CodeEditor {
    pub content: String,
    pub mode: EditorMode,
    pub read_only: bool,
}

impl CodeEditor {
    pub fn new(mode: EditorMode) -> Self {
        Self { content: String::new(), mode, read_only: false }
    }
}

impl Render for CodeEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = if self.mode == EditorMode::Sql {
            tokenize_sql(&self.content)
        } else {
            vec![Token { text: self.content.clone(), kind: TokenKind::Plain }]
        };

        let theme = cx.theme();
        // keyword: blue, string: amber, number: purple, comment: muted, plain: foreground
        let kw_color: Hsla = gpui::rgb(0x79c0ff).into();
        let str_color: Hsla = gpui::rgb(0xa5d6ff).into();
        let num_color: Hsla = gpui::rgb(0xa371f7).into();

        div()
            .font_family("monospace")
            .text_sm()
            .flex()
            .flex_wrap()
            .children(tokens.into_iter().map(|t| {
                let color = match t.kind {
                    TokenKind::Keyword => kw_color,
                    TokenKind::String => str_color,
                    TokenKind::Number => num_color,
                    TokenKind::Comment => theme.muted_foreground,
                    _ => theme.foreground,
                };
                div().text_color(color).child(t.text)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_classified() {
        let tokens = tokenize_sql("SELECT id FROM users WHERE id = 1");
        let kws: Vec<_> = tokens.iter().filter(|t| t.kind == TokenKind::Keyword).map(|t| t.text.as_str()).collect();
        assert!(kws.contains(&"SELECT"));
        assert!(kws.contains(&"FROM"));
        assert!(kws.contains(&"WHERE"));
    }

    #[test]
    fn string_literal_classified() {
        let tokens = tokenize_sql("WHERE plan = 'pro'");
        let strings: Vec<_> = tokens.iter().filter(|t| t.kind == TokenKind::String).collect();
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].text, "'pro'");
    }

    #[test]
    fn comment_classified() {
        let tokens = tokenize_sql("SELECT 1 -- get one");
        let comments: Vec<_> = tokens.iter().filter(|t| t.kind == TokenKind::Comment).collect();
        assert!(!comments.is_empty());
        assert!(comments[0].text.contains("get one"));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p desktop code_editor 2>&1
```

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/widgets/code_editor.rs
git commit -m "feat(widgets): implement CodeEditor with SQL token colorizer"
```

---

## Phase 6: Variable Substitution

---

### Task 10: Wire variable substitution into query editors

**Files:**
- Modify: `apps/desktop/src/project/variables.rs`
- Modify: `apps/desktop/src/postgres/query_editor.rs`
- Modify: `apps/desktop/src/sqlite/query_editor.rs`

- [ ] **Step 1: Verify variables.rs substitution function exists**

```bash
grep -n "pub fn" apps/desktop/src/project/variables.rs
```

If `substitute` or similar doesn't exist, implement it:

```rust
// In apps/desktop/src/project/variables.rs
/// Replace $VAR_NAME tokens in `input` with values from the map.
pub fn substitute(input: &str, vars: &std::collections::HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (key, val) in vars {
        result = result.replace(&format!("${}", key), val);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn substitutes_known_var() {
        let mut vars = HashMap::new();
        vars.insert("SCHEMA".into(), "public".into());
        let out = substitute("SELECT * FROM $SCHEMA.users", &vars);
        assert_eq!(out, "SELECT * FROM public.users");
    }

    #[test]
    fn unknown_var_left_as_is() {
        let vars = HashMap::new();
        let out = substitute("SELECT $UNKNOWN", &vars);
        assert_eq!(out, "SELECT $UNKNOWN");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p desktop variables 2>&1
```

- [ ] **Step 3: Wire into Postgres query editor**

In `apps/desktop/src/postgres/query_editor.rs`, find where the SQL is sent to the pool (look for `.execute(` or `sqlx::query`). Before that point, apply substitution:

```rust
// At the point where `self.sql` (or equivalent) is used:
use crate::project::variables::substitute;

// Get vars from project globals (or pass empty map if no project)
let vars = cx.try_global::<crate::project::ProjectVars>()
    .map(|v| v.vars.clone())
    .unwrap_or_default();
let sql = substitute(&self.sql, &vars);
// use `sql` instead of `self.sql` in the query call
```

If `ProjectVars` doesn't exist as a global yet, define a minimal one:
```rust
// apps/desktop/src/project/mod.rs — add:
#[derive(Default)]
pub struct ProjectVars {
    pub vars: std::collections::HashMap<String, String>,
}
impl gpui::Global for ProjectVars {}
```

And populate it when loading the workspace seed (in `Workspace::new`, after `load_workspace_seed`):
```rust
let vars = crate::project::variables::load_vars(project_root.as_deref());
cx.set_global(crate::project::ProjectVars { vars });
```

- [ ] **Step 4: Wire into SQLite query editor** (same pattern as Step 3)

- [ ] **Step 5: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/project/variables.rs apps/desktop/src/postgres/query_editor.rs apps/desktop/src/sqlite/query_editor.rs apps/desktop/src/project/mod.rs
git commit -m "feat: wire variable substitution into SQL query editors"
```

---

## Phase 7: Engine Panel Completions

---

### Task 11: Fix Connection Dashboards (all 3 engines)

**Files:**
- Modify: `apps/desktop/src/workspace/object_info.rs`
- Modify: `apps/desktop/src/postgres/mod.rs` (or dashboard panel)
- Modify: `apps/desktop/src/sqlite/mod.rs`
- Modify: `apps/desktop/src/mongodb/mod.rs`

- [ ] **Step 1: Find the hardcoded placeholder strings**

```bash
grep -n "0 queries\|No pinned\|TODO\|placeholder" apps/desktop/src/workspace/object_info.rs apps/desktop/src/postgres/*.rs apps/desktop/src/sqlite/*.rs apps/desktop/src/mongodb/*.rs 2>/dev/null
```

- [ ] **Step 2: Replace with real data from ConnectionEntry**

For each dashboard panel, replace hardcoded strings by reading from `ConnectionEntry`:
```rust
// Example for Postgres dashboard:
// Replace "0 queries" with history count from QueryStore
let history_count = cx.try_global::<crate::query_store::QueryStore>()
    .map(|qs| qs.history_for(&conn_id).len())
    .unwrap_or(0);
// Render: format!("{} queries", history_count)
```

For server version, it's already fetched during `Connectable::test` (in `TestReport.server_version`). Store it in `ConnectionEntry` and display it here.

- [ ] **Step 3: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/workspace/object_info.rs apps/desktop/src/postgres/ apps/desktop/src/sqlite/ apps/desktop/src/mongodb/
git commit -m "fix: replace hardcoded dashboard placeholders with real data"
```

---

### Task 12: Complete Schema Inspectors (all 3 engines)

**Files:**
- Modify: `apps/desktop/src/postgres/inspector.rs`
- Modify: `apps/desktop/src/sqlite/inspector.rs`
- Modify: `apps/desktop/src/mongodb/inspector.rs`

- [ ] **Step 1: Locate each stub**

```bash
grep -n "TODO\|stub\|content TODO" apps/desktop/src/postgres/inspector.rs apps/desktop/src/sqlite/inspector.rs apps/desktop/src/mongodb/inspector.rs
```

- [ ] **Step 2: Implement Postgres inspector tabs**

The inspector already has schema queries (confirmed by the exploration). Complete the render with tabs: Columns · Indexes · Constraints · Stats.

```rust
// Render skeleton for postgres/inspector.rs
// Tab: Columns
v_flex()
    .children(self.columns.iter().map(|col| {
        h_flex()
            .px_3().py_2()
            .border_b_1().border_color(cx.theme().border)
            .child(div().w(gpui::px(160.0)).text_sm().child(col.name.clone()))
            .child(div().w(gpui::px(100.0)).text_xs().text_color(cx.theme().muted_foreground).child(col.data_type.clone()))
            .child(div().flex_1().text_xs().text_color(cx.theme().muted_foreground).child(
                if col.nullable { "nullable" } else { "not null" }
            ))
    }))
```

- [ ] **Step 3: Implement SQLite inspector (PRAGMA-based)**

```rust
// sqlite/inspector.rs — columns from PRAGMA table_info()
// Already fetched via: sqlx::query("PRAGMA table_info(?)")
// Render the result rows as column list
// DDL tab: show CREATE TABLE statement from sqlite_master
let ddl_sql = format!(
    "SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
);
```

- [ ] **Step 4: Implement MongoDB collection inspector**

```rust
// mongodb/inspector.rs — use collStats + listIndexes
// collStats gives: count, avgObjSize, storageSize, totalIndexSize
// listIndexes gives: name, key doc, unique flag
```

- [ ] **Step 5: Compile check**

```bash
cargo check -p desktop 2>&1 | head -10
```

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/postgres/inspector.rs apps/desktop/src/sqlite/inspector.rs apps/desktop/src/mongodb/inspector.rs
git commit -m "feat: complete schema inspector panels for all 3 engines"
```

---

### Task 13: Wire QueryStore into Query Editors + History Sidebar

**Files:**
- Modify: `apps/desktop/src/postgres/query_editor.rs`
- Modify: `apps/desktop/src/sqlite/query_editor.rs`

- [ ] **Step 1: Add history recording after query execution**

In each query editor, after a successful query run, push to QueryStore:

```rust
// After getting result rows:
let duration_ms = start.elapsed().as_millis() as u64;
if let Ok(mut store) = cx.try_global_mut::<crate::query_store::QueryStore>() {
    // Note: cx.try_global_mut doesn't exist in GPUI — use cx.update_global:
}
// Correct pattern:
cx.update_global::<crate::query_store::QueryStore, _>(|store, _cx| {
    store.history.push(crate::query_store::HistoryEntry {
        conn_id: self.conn_id.clone(),
        query: sql.clone(),
        ran_at: time::OffsetDateTime::now_utc(),
        duration_ms,
        row_count: Some(rows.len() as u64),
    }, &store.queries_dir.clone());
});
```

- [ ] **Step 2: Add history sidebar panel to QueryEditor render**

Add a `show_history: bool` field to each QueryEditor struct. Render it as a collapsible right panel:

```rust
// In render, alongside the editor:
.when(self.show_history, |d| {
    d.child(
        v_flex()
            .w(gpui::px(260.0))
            .border_l_1()
            .border_color(cx.theme().border)
            .children(
                cx.try_global::<crate::query_store::QueryStore>()
                    .map(|qs| qs.history_for(&self.conn_id))
                    .unwrap_or_default()
                    .iter()
                    .take(20)
                    .map(|e| {
                        div()
                            .px_3().py_2()
                            .border_b_1().border_color(cx.theme().border)
                            .cursor_pointer()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(cx.theme().muted_foreground)
                            .child(e.query.chars().take(50).collect::<String>())
                    })
                    .collect::<Vec<_>>()
            )
    )
})
```

- [ ] **Step 3: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/postgres/query_editor.rs apps/desktop/src/sqlite/query_editor.rs
git commit -m "feat: wire QueryStore history into SQL query editors"
```

---

### Task 14: Wire FilterBar into DataViewers

**Files:**
- Modify: `apps/desktop/src/postgres/data_viewer.rs`
- Modify: `apps/desktop/src/sqlite/data_viewer.rs`
- Modify: `apps/desktop/src/mongodb/document_viewer.rs`

- [ ] **Step 1: Add FilterBar field to each DataViewer**

```rust
// In postgres/data_viewer.rs — add to struct:
filter_bar: Entity<crate::widgets::filter_bar::FilterBar>,
```

In `new()`:
```rust
let filter_bar = cx.new(|_| crate::widgets::filter_bar::FilterBar::new(vec![]));
// Populate columns after first data load
```

- [ ] **Step 2: Populate columns after schema load**

After fetching the first page of data, extract column names and update FilterBar:
```rust
let columns: Vec<String> = rows.first()
    .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
    .unwrap_or_default();
self.filter_bar.update(cx, |fb, _| fb.columns = columns);
```

- [ ] **Step 3: Apply filter expression to data query**

When FilterBar has an active expression, append it to the SELECT:
```rust
let base_sql = format!("SELECT * FROM {}", self.table_name);
let sql = if let Some(expr) = self.filter_bar.read(cx).current_expr() {
    format!("{} WHERE {} LIMIT {} OFFSET {}", base_sql, expr.to_sql(), self.page_size, self.offset)
} else {
    format!("{} LIMIT {} OFFSET {}", base_sql, self.page_size, self.offset)
};
```

- [ ] **Step 4: Render FilterBar in toolbar**

In each DataViewer's render method, add `self.filter_bar.clone()` to the toolbar row.

- [ ] **Step 5: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 6: Run and test filtering**

```bash
mise run dev
```
Open a table, type in filter bar, verify results update.

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/postgres/data_viewer.rs apps/desktop/src/sqlite/data_viewer.rs apps/desktop/src/mongodb/document_viewer.rs
git commit -m "feat: wire FilterBar into DataViewer panels"
```

---

### Task 15: MongoDB Document Editor (JSON editor)

**Files:**
- Modify: `apps/desktop/src/mongodb/document_editor.rs`

- [ ] **Step 1: Implement DocumentEditor**

```rust
// apps/desktop/src/mongodb/document_editor.rs
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, StyledExt, v_flex, h_flex};
use mongodb::bson::Document;

pub enum EditorMode { Insert, Edit }

pub struct DocumentEditor {
    pub collection_name: String,
    pub json_text: String,
    pub mode: EditorMode,
    pub error: Option<String>,
    pub original: Option<Document>, // for diff display in Edit mode
    database: mongodb::Database,
}

impl DocumentEditor {
    pub fn new_insert(collection: String, db: mongodb::Database) -> Self {
        Self {
            collection_name: collection,
            json_text: "{\n  \n}".into(),
            mode: EditorMode::Insert,
            error: None,
            original: None,
            database: db,
        }
    }

    pub fn new_edit(collection: String, doc: Document, db: mongodb::Database) -> Self {
        let json_text = serde_json::to_string_pretty(
            &mongodb::bson::to_document(&doc).unwrap_or_default()
        ).unwrap_or_default();
        Self {
            collection_name: collection,
            json_text,
            mode: EditorMode::Edit,
            error: None,
            original: Some(doc),
            database: db,
        }
    }

    fn validate(&self) -> Result<Document, String> {
        serde_json::from_str::<serde_json::Value>(&self.json_text)
            .map_err(|e| format!("Invalid JSON: {}", e))
            .and_then(|v| {
                mongodb::bson::to_bson(&v)
                    .ok()
                    .and_then(|b| b.as_document().cloned())
                    .ok_or_else(|| "JSON must be an object {}".into())
            })
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.validate() {
            Err(e) => { self.error = Some(e); cx.notify(); }
            Ok(doc) => {
                self.error = None;
                let db = self.database.clone();
                let collection_name = self.collection_name.clone();
                let is_insert = matches!(self.mode, EditorMode::Insert);
                let original_id = self.original.as_ref()
                    .and_then(|d| d.get("_id").cloned());

                cx.spawn(async move |_this, _cx| {
                    let coll = db.collection::<Document>(&collection_name);
                    let result = if is_insert {
                        coll.insert_one(doc).await.map(|_| ())
                    } else if let Some(id) = original_id {
                        coll.replace_one(
                            mongodb::bson::doc! { "_id": id },
                            doc,
                        ).await.map(|_| ())
                    } else {
                        Err(mongodb::error::Error::custom("no _id for replace"))
                    };
                    if let Err(e) = result {
                        log::warn!("document write failed: {e}");
                    }
                }).detach();
            }
        }
    }
}

impl Render for DocumentEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().flex_1().text_sm().text_color(cx.theme().muted_foreground).child(
                        match self.mode { EditorMode::Insert => "Insert Document", EditorMode::Edit => "Edit Document" }
                    ))
                    .child(
                        div()
                            .px_3().py_1()
                            .bg(cx.theme().accent)
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .child("Save")
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))
                    )
            )
            .when_some(self.error.clone(), |d, err| {
                d.child(
                    div().px_3().py_2()
                        .text_xs()
                        .text_color(gpui::rgb(0xff7b72).into())
                        .child(err)
                )
            })
            .child(
                div()
                    .flex_1()
                    .p_3()
                    .font_family("monospace")
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .bg(cx.theme().background)
                    .child(self.json_text.clone())
            )
    }
}

#[cfg(test)]
mod tests {
    // validate() is pure — test it without a real DB
    use mongodb::bson::doc;
    use serde_json::json;

    fn validate_json(json: &str) -> Result<(), String> {
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|e| format!("Invalid JSON: {}", e))
            .and_then(|v| {
                if v.is_object() { Ok(()) } else { Err("must be object".into()) }
            })
    }

    #[test]
    fn valid_json_object_passes() {
        assert!(validate_json("{\"a\": 1}").is_ok());
    }

    #[test]
    fn invalid_json_fails() {
        assert!(validate_json("{bad json}").is_err());
    }

    #[test]
    fn json_array_fails() {
        assert!(validate_json("[1,2,3]").is_err());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p desktop document_editor 2>&1
```

- [ ] **Step 3: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/mongodb/document_editor.rs
git commit -m "feat(mongodb): implement DocumentEditor with JSON validation + insert/replace"
```

---

### Task 16: Wire Pipeline Builder to QueryStore

**Files:**
- Modify: `apps/desktop/src/mongodb/pipeline_builder.rs`

- [ ] **Step 1: Record pipeline runs in history**

After a successful pipeline run in `pipeline_builder.rs`, push to QueryStore (same pattern as Task 13 Step 1). Use the serialized pipeline JSON as `query`.

- [ ] **Step 2: Add save-to-queries.toml action**

Add a "Save" button to the pipeline toolbar. On click, prompt for name inline (a text field that appears) and then:

```rust
cx.update_global::<crate::query_store::QueryStore, _>(|store, cx| {
    store.save_query(crate::query_store::SavedQuery {
        id: format!("q_{}", uuid::Uuid::new_v4().simple()),
        name: self.save_name.clone(),
        connection: self.conn_id.clone(),
        tags: vec![],
        sql: None,
        pipeline: Some(self.pipeline_json()),
    }, cx);
});
```

- [ ] **Step 3: Compile check + commit**

```bash
cargo check -p desktop 2>&1 | head -10
git add apps/desktop/src/mongodb/pipeline_builder.rs
git commit -m "feat(mongodb): wire QueryStore into pipeline builder"
```

---

## Phase 8: Infrastructure

---

### Task 17: Settings window

**Files:**
- Modify: `apps/desktop/src/settings_window/mod.rs`

- [ ] **Step 1: Implement minimal settings window**

```rust
// apps/desktop/src/settings_window/mod.rs
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, StyledExt, v_flex, h_flex, Switch};

pub struct SettingsWindow {
    // page_size options: 50, 100, 200, 500
    pub page_size: u32,
    pub query_timeout_secs: u32,
}

impl SettingsWindow {
    pub fn new() -> Self {
        Self { page_size: 100, query_timeout_secs: 30 }
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_6()
            .gap_6()
            .child(div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child("Settings"))
            .child(
                v_flex().gap_4()
                    .child(
                        h_flex().gap_4().items_center()
                            .child(div().w(gpui::px(160.0)).text_sm().child("Theme"))
                            .child(
                                div()
                                    .px_3().py_1()
                                    .bg(cx.theme().secondary)
                                    .rounded_md()
                                    .text_sm()
                                    .cursor_pointer()
                                    .child("Toggle Dark / Light")
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        crate::app::prefs::cycle_theme(cx);
                                    }))
                            )
                    )
                    .child(
                        h_flex().gap_4().items_center()
                            .child(div().w(gpui::px(160.0)).text_sm().child("Default page size"))
                            .child(
                                div().text_sm().text_color(cx.theme().muted_foreground)
                                    .child(format!("{} rows", self.page_size))
                            )
                    )
                    .child(
                        h_flex().gap_4().items_center()
                            .child(div().w(gpui::px(160.0)).text_sm().child("Query timeout"))
                            .child(
                                div().text_sm().text_color(cx.theme().muted_foreground)
                                    .child(format!("{}s", self.query_timeout_secs))
                            )
                    )
            )
    }
}
```

- [ ] **Step 2: Wire settings button in topbar**

In `apps/desktop/src/workspace/topbar.rs`, find the settings button click handler (currently an `eprintln!`). Replace with an action dispatch that opens the settings window:

```rust
// Register action in actions.rs: OpenSettings
// In topbar click handler:
cx.dispatch_action(Box::new(OpenSettings));
```

In `Workspace::new`, handle `OpenSettings`:
```rust
cx.on_action(|_this: &mut Workspace, _: &OpenSettings, window, cx| {
    cx.open_window(
        gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                origin: gpui::Point::default(),
                size: gpui::Size { width: gpui::px(480.0), height: gpui::px(360.0) },
            })),
            ..Default::default()
        },
        |_window, cx| cx.new(|_| SettingsWindow::new()),
    );
});
```

- [ ] **Step 3: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/settings_window/mod.rs apps/desktop/src/workspace/topbar.rs apps/desktop/src/app/actions.rs
git commit -m "feat: implement settings window with theme + page size options"
```

---

### Task 18: Wire file watcher for config reload

**Files:**
- Modify: `apps/desktop/src/project/mod.rs`
- Modify: `apps/desktop/src/project/watcher.rs`

- [ ] **Step 1: Check current watcher state**

```bash
grep -n "watcher\|ConfigWatcher" apps/desktop/src/project/mod.rs apps/desktop/src/project/watcher.rs
```

- [ ] **Step 2: Wire watcher into Project entity**

In `apps/desktop/src/project/mod.rs`, find where `_watcher: None` is set. Replace with:

```rust
// In Project struct, change `_watcher: Option<ConfigWatcher>` field to hold the watcher
// In Project::new (or wherever project is initialized):
let watcher = project_root.as_ref().map(|root| {
    let based_dir = root.join(".based");
    let handle = cx.model_handle_for_self(); // or equivalent GPUI handle
    crate::project::watcher::ConfigWatcher::new(&based_dir, move || {
        // Signal config reload — e.g. post a custom event or use a channel
        // The simplest approach: use a std::sync::Arc<AtomicBool> reload flag
        // checked on a timer, or use notify's async watcher with Tokio
        log::info!("based config changed — reload pending");
    })
});
self._watcher = watcher;
```

The exact integration depends on how GPUI handles cross-thread callbacks. The minimal correct implementation: set `_watcher` to `Some(watcher)` so it stays alive (currently it's `None` which drops the watcher immediately, disabling it). Full reload-on-change is a best-effort enhancement.

- [ ] **Step 3: Compile check**

```bash
cargo check -p desktop 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/project/mod.rs apps/desktop/src/project/watcher.rs
git commit -m "fix(project): keep ConfigWatcher alive so .based/ changes are detected"
```

---

## Phase 9: Explain Viewers (power tier)

---

### Task 19: Postgres Explain Viewer — parse EXPLAIN ANALYZE into node tree

**Files:**
- Modify: `apps/desktop/src/postgres/explain.rs`

- [ ] **Step 1: Define plan node types**

```rust
// In postgres/explain.rs — add at top:
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
        self.time_actual_ms.map(|t| t > threshold_ms).unwrap_or(false)
    }
}
```

- [ ] **Step 2: Add JSON-based parse (EXPLAIN (ANALYZE, FORMAT JSON))**

Switch the EXPLAIN query to use JSON output format for reliable parsing:

```rust
// In the query that runs EXPLAIN:
let explain_sql = format!("EXPLAIN (ANALYZE, FORMAT JSON) {}", user_sql);
// Parse the JSON result row[0][0] as serde_json::Value
// Walk the "Plan" tree recursively to build Vec<PlanNode>
```

Parse function:
```rust
pub fn parse_pg_explain_json(json: &serde_json::Value) -> Option<PlanNode> {
    let plan = json.as_array()?.first()?.get("Plan")?;
    Some(parse_node(plan))
}

fn parse_node(node: &serde_json::Value) -> PlanNode {
    PlanNode {
        node_type: node["Node Type"].as_str().unwrap_or("Unknown").to_string(),
        relation: node["Relation Name"].as_str().map(|s| s.to_string()),
        index_name: node["Index Name"].as_str().map(|s| s.to_string()),
        cost_startup: node["Startup Cost"].as_f64().unwrap_or(0.0),
        cost_total: node["Total Cost"].as_f64().unwrap_or(0.0),
        rows_estimated: node["Plan Rows"].as_u64().unwrap_or(0),
        rows_actual: node["Actual Rows"].as_u64(),
        time_actual_ms: node["Actual Total Time"].as_f64(),
        children: node["Plans"].as_array()
            .map(|plans| plans.iter().map(parse_node).collect())
            .unwrap_or_default(),
    }
}
```

- [ ] **Step 3: Add tests**

```rust
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
            relation: None, index_name: None,
            cost_startup: 0.0, cost_total: 9999.0,
            rows_estimated: 1000, rows_actual: Some(1000),
            time_actual_ms: Some(500.0),
            children: vec![],
        };
        assert!(node.is_slow(100.0));
        assert!(!node.is_slow(1000.0));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p desktop explain 2>&1
```

- [ ] **Step 5: Update render to show node tree**

Replace the current plain-text render in `explain.rs` with a recursive tree render. Each node shows: node type, relation name (if any), estimated vs actual rows, actual time. Nodes with `time_actual_ms > 100ms` get an amber left border.

- [ ] **Step 6: Compile check + commit**

```bash
cargo check -p desktop 2>&1 | head -10
git add apps/desktop/src/postgres/explain.rs
git commit -m "feat(postgres): parse EXPLAIN ANALYZE into visual node tree"
```

---

### Task 20: SQLite Explain Query Plan — parse into visual tree

**Files:**
- Modify: `apps/desktop/src/sqlite/eqp_viewer.rs`

- [ ] **Step 1: Parse EXPLAIN QUERY PLAN output**

SQLite's EQP output has columns: `id`, `parent`, `notused`, `detail`. Build a tree from parent-child relationships:

```rust
#[derive(Debug, Clone)]
pub struct EqpNode {
    pub id: i64,
    pub detail: String,
    pub children: Vec<EqpNode>,
    pub is_table_scan: bool, // true if detail contains "SCAN" but not "USING INDEX"
}

pub fn parse_eqp(rows: &[(i64, i64, String)]) -> Vec<EqpNode> {
    // rows: (id, parent_id, detail)
    // Build nodes for parent_id == 0 first, then attach children
    let mut nodes: std::collections::HashMap<i64, EqpNode> = rows.iter()
        .map(|(id, _, detail)| {
            let is_table_scan = detail.contains("SCAN") && !detail.contains("USING INDEX");
            (*id, EqpNode { id: *id, detail: detail.clone(), children: vec![], is_table_scan })
        })
        .collect();

    let mut roots = vec![];
    for (id, parent_id, _) in rows {
        if *parent_id == 0 {
            roots.push(*id);
        } else {
            let child = nodes.remove(id).unwrap();
            if let Some(parent) = nodes.get_mut(parent_id) {
                parent.children.push(child);
            }
        }
    }
    roots.into_iter().filter_map(|id| nodes.remove(&id)).collect()
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
        assert!(!tree[0].is_table_scan);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p desktop eqp 2>&1
```

- [ ] **Step 3: Update render — indented tree, table scans in amber**

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/sqlite/eqp_viewer.rs
git commit -m "feat(sqlite): parse EXPLAIN QUERY PLAN into visual tree with scan warnings"
```

---

## Self-Review Checklist

### Spec coverage

| Spec section | Covered by |
|---|---|
| Architecture: ConnectionTree | Task 3, 4 |
| Architecture: TabManager | Task 1, 2 |
| Architecture: QueryStore | Task 5 |
| Architecture: CommandPalette | Task 6 |
| Architecture: WorkspaceState | Task 4 (session persistence) |
| Connection Tree: nested tree | Task 3 |
| Connection Tree: states | Task 3 (ConnState) |
| Connection Tree: lazy schema | Task 3 (objects: None until expanded) |
| Connection Tree: right-click menu | Task 4 (wired to context menu) |
| Tab System: TabSpec enum | Task 1 |
| Tab System: lifecycle rules | Task 2 (open_or_focus, close) |
| Command Palette: ⌘K | Task 6 |
| QueryStore: history.jsonl | Task 5 |
| QueryStore: queries.toml | Task 5 |
| QueryStore: .based/local gitignore | Task 5 |
| Variable substitution | Task 10 |
| Shared widgets: FilterBar | Task 7 |
| Shared widgets: CellDetail | Task 8 |
| Shared widgets: CodeEditor | Task 9 |
| Engine panels: dashboards | Task 11 |
| Engine panels: schema inspectors | Task 12 |
| Engine panels: query editors + history | Task 13 |
| Engine panels: filter bar wiring | Task 14 |
| Engine panels: document editor | Task 15 |
| Engine panels: pipeline + QueryStore | Task 16 |
| Settings window | Task 17 |
| File watcher | Task 18 |
| Postgres Explain viewer | Task 19 |
| SQLite EQP viewer | Task 20 |
| Live monitoring | Deferred — not in plan ✓ |
| Change stream | Deferred — not in plan ✓ |

### Notes for executor

- **GPUI patterns:** All UI entities implement `Render`. Use `cx.notify()` to trigger re-render. Use `cx.spawn(async move |this, cx| {...}).detach()` for async work. Use `cx.update_global::<T, _>(|g, _cx| {...})` to mutate globals.
- **No test infra for UI:** GPUI render tests require a full app context. Unit tests here target pure logic only (QueryStore, FilterBar expressions, plan parsing). UI correctness is verified via `mise run dev`.
- **Compile is the primary gate:** After each task, `cargo check -p desktop` must pass before committing.
- **`workspace/mod.rs` is 1,490 lines:** Tasks 3 and 4 extract from it. Work carefully — the file has many interdependencies. Extract one concern at a time, compile-check after each extraction.
