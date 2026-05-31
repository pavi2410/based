//! Command palette (⌘K / Ctrl+K): quick jump to connections, saved queries, and history.

use std::collections::HashSet;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, KeyBinding,
    MouseButton, Render, ScrollHandle, SharedString, Subscription, Window, actions, div, point,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName,
    input::{Input, InputEvent, InputState},
    scroll::Scrollbar,
    v_flex,
};

use crate::connection::registry::ConnectionRegistry;
use crate::connection::{ConnectionId, EngineKind};
use crate::query_store::QueryStore;
use crate::widgets::list_row::palette_result_row;
use crate::widgets::ui::palette_footer_hints;
use crate::workspace::connection_tree::ConnectionTree;
use crate::workspace::tab_spec::TabSpec;

const PALETTE_LIST_H: f32 = 360.0;
const PALETTE_ROW_H: f32 = 28.0;

actions!(command_palette, [PaletteSelectUp, PaletteSelectDown]);

#[derive(Clone, PartialEq, Eq, serde::Deserialize, gpui::Action)]
#[action(namespace = command_palette, no_json)]
struct PaletteConfirm {
    secondary: bool,
}

pub fn init(cx: &mut App) {
    let ctx = Some("CommandPalette");
    cx.bind_keys([
        KeyBinding::new("up", PaletteSelectUp, ctx),
        KeyBinding::new("down", PaletteSelectDown, ctx),
        KeyBinding::new("enter", PaletteConfirm { secondary: false }, ctx),
        KeyBinding::new("secondary-enter", PaletteConfirm { secondary: true }, ctx),
    ]);
}

/// Emitted when the user picks a palette row — workspace opens the tab.
#[derive(Clone, Debug)]
pub enum PaletteEvent {
    OpenTab(TabSpec),
    OpenProjectQuery(String),
    /// Load SQL into the active query editor when conn matches.
    InjectSql {
        conn_id: ConnectionId,
        sql: String,
    },
    WorkspaceAction(WorkspacePaletteAction),
}

#[derive(Clone, Debug)]
pub enum WorkspacePaletteAction {
    NewLooseQuery,
    NewCollection,
    SelectNoEnvironment,
    OpenWelcome,
    OpenOnboarding,
    CheckForUpdates,
    OpenProject,
    OpenProjectInNewWindow,
}

/// A search result the palette can return.
#[derive(Clone)]
#[allow(dead_code)]
pub struct PaletteResult {
    pub kind: ResultKind,
    pub label: String,
    pub sublabel: String,
    pub conn_label: String,
    pub spec: TabSpec,
    pub project_query_path: Option<String>,
    pub command_action: Option<WorkspacePaletteAction>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ResultKind {
    SchemaObject,
    SavedQuery,
    History,
    Command,
}

pub struct CommandPalette {
    registry: Entity<ConnectionRegistry>,
    connection_tree: Entity<ConnectionTree>,
    search_input: Entity<InputState>,
    results: Vec<PaletteResult>,
    selected: usize,
    visible: bool,
    focus_handle: FocusHandle,
    results_scroll: ScrollHandle,
    _search_subscription: Subscription,
}

impl CommandPalette {
    pub fn new(
        registry: Entity<ConnectionRegistry>,
        connection_tree: Entity<ConnectionTree>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Search tables, queries, history…"));
        let _search_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);

        Self {
            registry,
            connection_tree,
            search_input,
            results: vec![],
            selected: 0,
            visible: false,
            focus_handle: cx.focus_handle(),
            results_scroll: ScrollHandle::new(),
            _search_subscription,
        }
    }

    fn query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.visible = !self.visible;
        if self.visible {
            self.search_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            self.selected = 0;
            self.results_scroll.set_offset(point(px(0.0), px(0.0)));
            self.refresh_results(cx);
            self.search_input
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
        }
        cx.notify();
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.visible = false;
        cx.notify();
    }

    fn open_selected(&mut self, secondary: bool, cx: &mut Context<Self>) {
        let Some(entry) = self.results.get(self.selected) else {
            return;
        };
        match (&entry.kind, secondary) {
            (ResultKind::Command, _) => {
                if let Some(action) = entry.command_action.clone() {
                    cx.emit(PaletteEvent::WorkspaceAction(action));
                }
            }
            (ResultKind::History, false) => {
                let sql = match &entry.spec {
                    TabSpec::QueryEditor {
                        initial_sql: Some(s),
                        ..
                    } => s.clone(),
                    TabSpec::QueryEditor {
                        initial_pipeline: Some(p),
                        ..
                    } => p.clone(),
                    _ => entry.label.clone(),
                };
                cx.emit(PaletteEvent::InjectSql {
                    conn_id: entry.spec.conn_id().clone(),
                    sql,
                });
            }
            (ResultKind::SavedQuery, _) => {
                if let Some(path) = &entry.project_query_path {
                    cx.emit(PaletteEvent::OpenProjectQuery(path.clone()));
                } else {
                    cx.emit(PaletteEvent::OpenTab(entry.spec.clone()));
                }
            }
            _ => {
                let spec = match (&entry.kind, secondary) {
                    (ResultKind::SchemaObject, true) => {
                        let table = entry.label.clone();
                        TabSpec::QueryEditor {
                            conn_id: entry.spec.conn_id().clone(),
                            initial_sql: Some(format!("SELECT * FROM {table} LIMIT 100")),
                            initial_pipeline: None,
                            mongo_collection: None,
                            auto_run: false,
                        }
                    }
                    _ => entry.spec.clone(),
                };
                cx.emit(PaletteEvent::OpenTab(spec));
            }
        }
        self.dismiss(cx);
    }

    fn on_search_input_event(
        &mut self,
        _input: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            self.selected = 0;
            self.results_scroll.set_offset(point(px(0.0), px(0.0)));
            self.refresh_results(cx);
        }
    }

    fn select_prev(&mut self, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(1);
        self.scroll_to_selected();
        cx.notify();
    }

    fn select_next(&mut self, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        let max = self.results.len() - 1;
        self.selected = (self.selected + 1).min(max);
        self.scroll_to_selected();
        cx.notify();
    }

    fn scroll_to_selected(&self) {
        let row_top = px(self.selected as f32 * PALETTE_ROW_H);
        let row_bottom = row_top + px(PALETTE_ROW_H);
        let view_top = self.results_scroll.offset().y;
        let view_bottom = view_top + px(PALETTE_LIST_H);
        if row_top < view_top {
            self.results_scroll.set_offset(point(px(0.0), row_top));
        } else if row_bottom > view_bottom {
            self.results_scroll
                .set_offset(point(px(0.0), row_bottom - px(PALETTE_LIST_H)));
        }
    }

    fn refresh_results(&mut self, cx: &mut Context<Self>) {
        let q = self.query(cx).to_lowercase();
        let mut results = vec![];

        if q.is_empty()
            || q.contains("workspace")
            || q.contains("loose")
            || q.contains("collection")
        {
            results.push(PaletteResult {
                kind: ResultKind::Command,
                label: "New loose query".into(),
                sublabel: "workspace".into(),
                conn_label: String::new(),
                spec: TabSpec::blank_query_editor(ConnectionId("".into())),
                command_action: Some(WorkspacePaletteAction::NewLooseQuery),
                project_query_path: None,
            });
            results.push(PaletteResult {
                kind: ResultKind::Command,
                label: "New collection".into(),
                sublabel: "workspace".into(),
                conn_label: String::new(),
                spec: TabSpec::blank_query_editor(ConnectionId("".into())),
                command_action: Some(WorkspacePaletteAction::NewCollection),
                project_query_path: None,
            });
        }
        if q.is_empty() || q.contains("environment") || q.contains("no env") {
            results.push(PaletteResult {
                kind: ResultKind::Command,
                label: "Select No Environment".into(),
                sublabel: "environment".into(),
                conn_label: String::new(),
                spec: TabSpec::blank_query_editor(ConnectionId("".into())),
                command_action: Some(WorkspacePaletteAction::SelectNoEnvironment),
                project_query_path: None,
            });
        }
        if q.is_empty() || q.contains("welcome") {
            results.push(PaletteResult {
                kind: ResultKind::Command,
                label: "Open Welcome".into(),
                sublabel: "navigation".into(),
                conn_label: String::new(),
                spec: TabSpec::blank_query_editor(ConnectionId("".into())),
                command_action: Some(WorkspacePaletteAction::OpenWelcome),
                project_query_path: None,
            });
        }
        if q.is_empty() || q.contains("onboarding") || q.contains("setup") {
            results.push(PaletteResult {
                kind: ResultKind::Command,
                label: "Open Onboarding".into(),
                sublabel: "navigation".into(),
                conn_label: String::new(),
                spec: TabSpec::blank_query_editor(ConnectionId("".into())),
                command_action: Some(WorkspacePaletteAction::OpenOnboarding),
                project_query_path: None,
            });
        }
        if q.is_empty() || q.contains("project") || q.contains("folder") || q.contains("open") {
            results.push(PaletteResult {
                kind: ResultKind::Command,
                label: "Open Project".into(),
                sublabel: "project".into(),
                conn_label: String::new(),
                spec: TabSpec::blank_query_editor(ConnectionId("".into())),
                command_action: Some(WorkspacePaletteAction::OpenProject),
                project_query_path: None,
            });
            results.push(PaletteResult {
                kind: ResultKind::Command,
                label: "Open Project in New Window".into(),
                sublabel: "project".into(),
                conn_label: String::new(),
                spec: TabSpec::blank_query_editor(ConnectionId("".into())),
                command_action: Some(WorkspacePaletteAction::OpenProjectInNewWindow),
                project_query_path: None,
            });
        }
        if crate::app::prefs::manual_update_checks_enabled()
            && (q.is_empty() || q.contains("update"))
        {
            results.push(PaletteResult {
                kind: ResultKind::Command,
                label: "Check for Updates".into(),
                sublabel: "application".into(),
                conn_label: String::new(),
                spec: TabSpec::blank_query_editor(ConnectionId("".into())),
                command_action: Some(WorkspacePaletteAction::CheckForUpdates),
                project_query_path: None,
            });
        }

        let tree = self.connection_tree.read(cx);
        for (conn_id, obj, _engine) in tree.schema_palette_matches(&q, cx) {
            let display = obj.display_name();
            results.push(PaletteResult {
                kind: ResultKind::SchemaObject,
                label: display.clone(),
                sublabel: obj.kind.group().to_string(),
                conn_label: conn_id.0.clone(),
                spec: TabSpec::DataViewer {
                    conn_id: conn_id.clone(),
                    object: display,
                },
                command_action: None,
                project_query_path: None,
            });
        }

        let store = cx.global::<QueryStore>();
        for query in store.project_queries() {
            let hay = format!(
                "{} {} {}",
                query.name,
                query.description.as_deref().unwrap_or(""),
                query.tags.join(" ")
            )
            .to_lowercase();
            if q.is_empty() || hay.contains(&q) {
                let target = crate::workspace::project_query::target_hint(&query.target);
                results.push(PaletteResult {
                    kind: ResultKind::SavedQuery,
                    label: query.name.clone(),
                    sublabel: format!("query · {target}"),
                    conn_label: String::new(),
                    spec: TabSpec::Welcome,
                    project_query_path: Some(query.path.clone()),
                    command_action: None,
                });
            }
        }

        let mut seen_history: HashSet<(ConnectionId, String)> = HashSet::new();
        for entry in store.history.recent(100) {
            if q.is_empty() || entry.query.to_lowercase().contains(&q) {
                let key = (entry.conn_id.clone(), entry.query.trim().to_lowercase());
                if !seen_history.insert(key) {
                    continue;
                }
                let engine = self
                    .registry
                    .read(cx)
                    .get(&entry.conn_id, cx)
                    .map(|e| e.read(cx).config.engine());
                let spec = match engine {
                    Some(EngineKind::MongoDB) => TabSpec::QueryEditor {
                        conn_id: entry.conn_id.clone(),
                        initial_sql: None,
                        initial_pipeline: Some(entry.query.clone()),
                        mongo_collection: None,
                        auto_run: false,
                    },
                    _ => TabSpec::QueryEditor {
                        conn_id: entry.conn_id.clone(),
                        initial_sql: Some(entry.query.clone()),
                        initial_pipeline: None,
                        mongo_collection: None,
                        auto_run: false,
                    },
                };
                let meta = format!(
                    "history · {}",
                    entry
                        .ran_at
                        .format(&time::format_description::well_known::Rfc3339)
                        .unwrap_or_else(|_| "recent".into())
                );
                results.push(PaletteResult {
                    kind: ResultKind::History,
                    label: entry.query.chars().take(72).collect(),
                    sublabel: meta,
                    conn_label: entry.conn_id.0.clone(),
                    spec,
                    command_action: None,
                    project_query_path: None,
                });
            }
        }

        self.results = results;
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
        cx.notify();
    }
}

impl EventEmitter<PaletteEvent> for CommandPalette {}

impl Focusable for CommandPalette {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.visible {
            self.search_input.read(cx).focus_handle(cx)
        } else {
            self.focus_handle.clone()
        }
    }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().into_any_element();
        }

        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let fg = theme.foreground;

        div()
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x00000088))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.dismiss(cx);
                }),
            )
            .child(
                v_flex()
                    .absolute()
                    .top(gpui::px(120.0))
                    .left_1_2()
                    .ml(gpui::px(-280.0))
                    .w(gpui::px(560.0))
                    .max_h(gpui::px(480.0))
                    .overflow_hidden()
                    .track_focus(&self.focus_handle)
                    .key_context("CommandPalette")
                    .on_action(cx.listener(|this, _: &PaletteSelectUp, _, cx| {
                        this.select_prev(cx);
                    }))
                    .on_action(cx.listener(|this, _: &PaletteSelectDown, _, cx| {
                        this.select_next(cx);
                    }))
                    .on_action(cx.listener(|this, action: &PaletteConfirm, _, cx| {
                        this.open_selected(action.secondary, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        div()
                            .flex_shrink_0()
                            .p_2()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                Input::new(&self.search_input)
                                    .appearance(false)
                                    .p_0()
                                    .prefix(
                                        Icon::new(IconName::Search)
                                            .text_color(theme.muted_foreground),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .h(gpui::px(PALETTE_LIST_H))
                            .child(
                                div()
                                    .id("palette-results-scroll")
                                    .track_scroll(&self.results_scroll)
                                    .overflow_y_scroll()
                                    .size_full()
                                    .child(v_flex().children({
                                        let results: Vec<_> = self
                                            .results
                                            .iter()
                                            .enumerate()
                                            .map(|(i, r)| {
                                                let is_sel = i == self.selected;
                                                let conn_label: SharedString =
                                                    r.conn_label.clone().into();
                                                let label: SharedString = r.label.clone().into();
                                                let sublabel: SharedString =
                                                    r.sublabel.clone().into();
                                                (i, is_sel, conn_label, label, sublabel)
                                            })
                                            .collect();
                                        results.into_iter().map(
                                            |(i, is_sel, conn_label, label, sublabel)| {
                                                palette_result_row(
                                                    ("palette-result", i),
                                                    is_sel,
                                                    conn_label,
                                                    label,
                                                    sublabel,
                                                    muted,
                                                    fg,
                                                )
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        cx.stop_propagation();
                                                        this.selected = i;
                                                        this.open_selected(false, cx);
                                                    }),
                                                )
                                            },
                                        )
                                    })),
                            )
                            .child(Scrollbar::vertical(&self.results_scroll)),
                    )
                    .child(palette_footer_hints(window, cx)),
            )
            .into_any_element()
    }
}
