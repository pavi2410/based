// Paginated table viewer shared by SQL engines; engines plug in via [`DataSource`].

use crate::workspace::connection_health::mark_connection_lost;
use gpui_kit::assets::IconName as AssetIcon;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

use futures::future::BoxFuture;
use gpui_kit::component::table::TableEvent;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{BasePanel, Panel, PanelEvent},
    menu::{DropdownMenu as _, PopupMenu, PopupMenuItem},
    table::{Column, TableState},
    v_flex,
};
use gpui_kit::{prelude::*, *};

use crate::ai::explain::{column_prompt, distinct_samples};
use crate::ai::{ask_assistant, connection_ai};
use crate::app::prefs;
use crate::connection::ConnectionId;
use crate::connection::is_connection_read_only;
use crate::db;
use crate::widgets::cell_detail::CellDetail;
use crate::widgets::column_header::ColumnMeta;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::datetime::format_relative;
use crate::widgets::empty_state::status_panel;
use crate::widgets::export_popover::export_popover;
use crate::widgets::filter_bar::{FilterBar, FilterEvent, FilterExpr, FilterOp, FilterSet};
use crate::widgets::motion;
use crate::widgets::pagination::{offset_for_page, sql_pagination_group};
use crate::widgets::panel::{
    data_viewer_toolbar, panel_tab_content, tab_breadcrumb_data_viewer_trailing,
    tab_breadcrumb_footer, tab_breadcrumb_for_connection,
};
use crate::widgets::virtual_table::{
    Cell, RowDelegate, align_meta_to_columns, cells, column_visibility, data_column,
    replace_table_data, sort_column, toggle_column,
};
use crate::workspace::WorkspaceRef;
use crate::workspace::pop_out::PopOutWindowTitle;
use crate::workspace::tabs::{DrillFilter, TabSpec, inbox, request_workspace_flush};
use based_ai::context::ContextLevel;
use based_core::{ColumnCatalog, ObjectRef, PageSort, RawPage, Value};

mod menus;

fn is_array_type(ty: Option<&str>) -> bool {
    ty.is_some_and(|t| t.trim().ends_with("[]") || t.trim().eq_ignore_ascii_case("array"))
}

/// Names of columns whose catalog type is an array.
fn meta_array_columns(names: &[String], meta: &[ColumnMeta]) -> HashSet<String> {
    names
        .iter()
        .zip(meta)
        .filter(|(_, m)| is_array_type(m.data_type.as_deref()))
        .map(|(n, _)| n.clone())
        .collect()
}

pub type Fut<T> = BoxFuture<'static, anyhow::Result<T>>;

/// One page of table rows.
pub struct PageData {
    pub total: i64,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
}

impl PageData {
    /// An empty page has no result columns; it shows the table's, in table order.
    pub fn new(page: RawPage, catalog: &ColumnCatalog) -> Self {
        let names: Vec<String> = if page.set.columns.is_empty() {
            catalog.names().map(str::to_string).collect()
        } else {
            page.set.column_names().map(str::to_string).collect()
        };
        let columns = names
            .into_iter()
            .map(|n| data_column(n.clone(), n))
            .collect();
        Self {
            total: page.total,
            columns,
            rows: cells(page.set.rows),
        }
    }
}

/// Double-quote a SQL identifier.
/// Engine-specific data access for [`DataViewerPanel`].
pub trait DataSource: Send + Sync + 'static {
    /// Dock panel name, e.g. `"PgDataViewer"`.
    fn panel_name(&self) -> &'static str;
    /// Widget id prefix, e.g. `"pg-dv"`.
    fn id(&self) -> &'static str;
    /// Object path shown in breadcrumbs and titles, e.g. `["public", "users"]`.
    fn path(&self) -> Vec<String>;
    fn catalog(&self) -> Fut<ColumnCatalog>;
    fn page(
        &self,
        filter: Option<FilterSet>,
        sort: Option<PageSort>,
        offset: u64,
        limit: u64,
        catalog: ColumnCatalog,
    ) -> Fut<PageData>;
    /// Whether a failed load means the connection itself is gone.
    fn lost_connection(&self, _err: &anyhow::Error) -> bool {
        false
    }
}

pub struct DataViewerPanel {
    focus_handle: FocusHandle,
    source: Arc<dyn DataSource>,
    conn_id: ConnectionId,
    pub(super) table: Entity<TableState<RowDelegate>>,
    cell_detail: Entity<CellDetail>,
    filter_bar: Entity<FilterBar>,
    pub(super) offset: u64,
    pub(super) page_size: u64,
    pub(super) total_rows: u64,
    pub(super) loading: bool,
    last_load_ms: Option<u64>,
    last_fetched: Option<OffsetDateTime>,
    /// First line of the last load failure; cleared by the next load.
    load_error: Option<SharedString>,
    column_catalog: ColumnCatalog,
    /// Server-side sort chosen in the grid; `None` pages by primary key.
    sort: Option<PageSort>,
    /// Foreign-key jump for the selected cell.
    fk_jump: Option<(ObjectRef, DrillFilter)>,
    self_handle: WeakEntity<Self>,
    pub(crate) tab_label: SharedString,
}

impl DataViewerPanel {
    pub fn new(
        source: Arc<dyn DataSource>,
        conn_id: ConnectionId,
        filter: Option<DrillFilter>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let table = cx.new(|cx| configure_row_table(RowDelegate::default(), window, cx));
        let filter_bar = cx.new(|cx| FilterBar::new(window, cx, vec![]));
        if let Some(f) = filter {
            filter_bar.update(cx, |fb, cx| fb.preset_eq(f.column, &f.value, window, cx));
        }
        cx.subscribe(&filter_bar, |panel, _, _: &FilterEvent, cx| {
            panel.load_page(0, cx)
        })
        .detach();
        let cell_detail = cx.new(|_| CellDetail::default());
        let tab_label = source.path().join(".").into();
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            source,
            conn_id,
            table,
            cell_detail,
            filter_bar,
            offset: 0,
            page_size: prefs::page_size(cx),
            total_rows: 0,
            loading: false,
            last_load_ms: None,
            last_fetched: None,
            load_error: None,
            column_catalog: ColumnCatalog::default(),
            sort: None,
            fk_jump: None,
            self_handle: cx.weak_entity(),
            tab_label,
        };
        cx.subscribe(&panel.table, |panel, _, event, cx| {
            if let TableEvent::SelectCell(row, col) = event {
                panel.fk_jump = panel.fk_target_for(*row, *col, cx);
                cx.notify();
            }
            if let TableEvent::DoubleClickedCell(row, col) = event {
                let Some((col_name, val)) = panel.cell_snapshot(*row, *col, cx) else {
                    return;
                };
                panel.cell_detail.update(cx, |d, cx| {
                    d.show(col_name, val);
                    cx.notify();
                });
                cx.notify();
            }
        })
        .detach();
        let weak = cx.weak_entity();
        let header_weak = cx.weak_entity();
        let sort_weak = cx.weak_entity();
        panel.table.update(cx, |t, _| {
            t.delegate_mut().server_sort = Some(Rc::new(move |sort, cx| {
                let _ = sort_weak.update(cx, |panel, cx| {
                    panel.sort = sort.map(|(column, ascending)| PageSort {
                        column: column.to_string(),
                        ascending,
                    });
                    panel.load_page(0, cx);
                });
            }));
            t.delegate_mut().header_menu = Some(Rc::new(move |menu, col, window, cx| {
                match header_weak.upgrade() {
                    Some(p) => Self::header_menu(&p, menu, col, window, cx),
                    None => menu,
                }
            }));
            t.delegate_mut().cell_menu =
                Some(Rc::new(move |menu, row, col, window, cx| {
                    match weak.upgrade() {
                        Some(p) => Self::cell_menu(&p, menu, row, col, window, cx),
                        None => menu,
                    }
                }));
        });
        panel.load_page(0, cx);
        // Keep the relative "fetched" label fresh.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        panel
    }

    fn load_page(&mut self, offset: u64, cx: &mut Context<Self>) {
        self.loading = true;
        self.load_error = None;
        self.offset = offset;
        let source = self.source.clone();
        let page_size = self.page_size;
        let filter = self.filter_bar.read(cx).current_set(cx);
        let sort = self.sort.clone();
        let cached = self.column_catalog.clone();

        cx.spawn(async move |this, cx| {
            let catalog = if cached.is_empty() {
                db::run(cx, source.catalog()).await.unwrap_or_default()
            } else {
                cached
            };
            let fetch = source.page(filter, sort, offset, page_size, catalog.clone());
            let res = db::run(cx, async move {
                let start = Instant::now();
                let page = fetch.await?;
                Ok((page, start.elapsed().as_millis() as u64))
            })
            .await;

            let _ = this.update(cx, |panel, cx| {
                panel.loading = false;
                if let Err(e) = &res {
                    if panel.source.lost_connection(e) {
                        mark_connection_lost(&panel.conn_id, &e.to_string(), cx);
                    }
                    let msg = e.to_string();
                    panel.load_error = Some(
                        msg.lines()
                            .next()
                            .unwrap_or("Unknown error")
                            .to_string()
                            .into(),
                    );
                }
                if let Ok((page, elapsed_ms)) = res {
                    panel.total_rows = page.total as u64;
                    panel.last_load_ms = Some(elapsed_ms);
                    panel.last_fetched = Some(OffsetDateTime::now_utc());
                    let names: Vec<String> =
                        page.columns.iter().map(|c| c.key.to_string()).collect();
                    let meta = align_meta_to_columns(names.iter().cloned(), &catalog);
                    panel.column_catalog = catalog;
                    let arrays = meta_array_columns(&names, &meta);
                    panel.filter_bar.update(cx, |fb, cx| {
                        fb.set_array_columns(arrays);
                        fb.set_columns_if_empty(names, cx);
                    });
                    panel.table.update(cx, |state, cx| {
                        replace_table_data(state, page.columns, page.rows, meta, cx);
                    });
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Error, empty-table and no-match screens drawn over the grid; `None` when rows show.
    fn state_overlay(
        &self,
        loading: bool,
        filtered: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if loading {
            return None;
        }
        let theme = cx.theme();
        let offset = self.offset;
        if let Some(err) = self.load_error.clone() {
            let copy = err.to_string();
            return Some(status_panel(
                "Couldn't load rows",
                "The query failed or timed out. Check the connection and try again.",
                IconName::TriangleAlert,
                Some(theme.danger),
                Some(err),
                vec![
                    Button::new("dv-retry")
                        .small()
                        .primary()
                        .label("Retry")
                        .on_click(cx.listener(move |p, _, _, cx| p.load_page(offset, cx)))
                        .into_any_element(),
                    Button::new("dv-copy-error")
                        .small()
                        .outline()
                        .label("Copy details")
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                        })
                        .into_any_element(),
                ],
                cx,
            ));
        }
        if self.last_fetched.is_none() || self.total_rows > 0 {
            return None;
        }
        Some(if filtered {
            status_panel(
                "No rows match",
                "The filter hides every row in this table.",
                IconName::Search,
                None,
                None,
                vec![
                    Button::new("dv-clear-filter")
                        .small()
                        .primary()
                        .label("Clear filter")
                        .on_click(cx.listener(|p, _, window, cx| {
                            p.filter_bar.update(cx, |fb, cx| fb.clear(window, cx));
                            p.load_page(0, cx);
                        }))
                        .into_any_element(),
                ],
                cx,
            )
        } else {
            status_panel(
                "This table is empty",
                "No rows yet.",
                IconName::Inbox,
                None,
                None,
                Vec::new(),
                cx,
            )
        })
    }

    pub(super) fn go_to_page(&mut self, page_1_based: usize, cx: &mut Context<Self>) {
        self.load_page(offset_for_page(page_1_based, self.page_size), cx);
    }
}

/// Toolbar menu listing every column with a check for the visible ones.
fn columns_button(table: &Entity<TableState<RowDelegate>>) -> impl IntoElement {
    let table = table.clone();
    Button::new("dv-columns")
        .small()
        .ghost()
        .icon(Icon::new(IconName::Eye))
        .label("Columns")
        .dropdown_caret(true)
        .dropdown_menu(move |menu, _, cx| {
            let cols = column_visibility(table.read(cx).delegate());
            cols.into_iter().fold(menu, |menu, (key, name, visible)| {
                let table = table.clone();
                menu.item(
                    PopupMenuItem::new(name)
                        .checked(visible)
                        .on_click(move |_, _, cx| {
                            table.update(cx, |t, cx| toggle_column(t, &key, cx));
                        }),
                )
            })
        })
}

/// Menu item that ANDs `column = value` (or `<>` when `negate`) onto the filter.
fn filter_item(
    label: &'static str,
    negate: bool,
    column: &str,
    cell: &Cell,
    this: &WeakEntity<DataViewerPanel>,
) -> PopupMenuItem {
    let (this, column) = (this.clone(), column.to_string());
    let is_null = cell.is_null();
    let value = cell.display.to_string();
    PopupMenuItem::new(label)
        .icon(Icon::new(if negate {
            AssetIcon::FunnelX
        } else {
            AssetIcon::Funnel
        }))
        .on_click(move |_, _, cx| {
            let op = match (is_null, negate) {
                (true, false) => FilterOp::IsNull,
                (true, true) => FilterOp::IsNotNull,
                (false, false) => FilterOp::Eq,
                (false, true) => FilterOp::NotEq,
            };
            let expr = FilterExpr {
                column: column.clone(),
                op,
                value: if is_null {
                    String::new()
                } else {
                    value.clone()
                },
            };
            let _ = this.update(cx, |p, cx| {
                p.filter_bar.update(cx, |fb, cx| fb.add_term(expr, cx));
            });
        })
}

/// Refresh glyph that spins while `spinning`.
fn refresh_icon(spinning: bool) -> impl IntoElement {
    let icon = Icon::new(IconName::RefreshCw).small();
    if !spinning {
        return icon.into_any_element();
    }
    icon.with_animation(
        "refresh-spin",
        Animation::new(Duration::from_millis(900)).repeat(),
        |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
    )
    .into_any_element()
}

impl EventEmitter<PanelEvent> for DataViewerPanel {}

impl Focusable for DataViewerPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for DataViewerPanel {
    fn panel_name(&self) -> &'static str {
        self.source.panel_name()
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for DataViewerPanel {
    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    crate::based_panel_tab_chrome!();
}

impl PopOutWindowTitle for DataViewerPanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        self.source.path().join(".")
    }
}

impl Render for DataViewerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (total, offset, page_size, loading) =
            (self.total_rows, self.offset, self.page_size, self.loading);
        let id = self.source.id();

        let panel = cx.entity().downgrade();
        let export_rows_empty = self.table.read(cx).delegate().rows.is_empty();
        let (export_headers, export_rows) = {
            let st = self.table.read(cx);
            let d = st.delegate();
            let h = d.columns.iter().map(|c| c.key.to_string()).collect();
            (h, d.rows.clone())
        };

        let toolbar = data_viewer_toolbar(cx)
            .child(self.filter_bar.clone())
            .child(columns_button(&self.table))
            .child(export_popover(id, export_headers, export_rows));

        let filtered = self.filter_bar.read(cx).current_set(cx).is_some();
        let overlay = self.state_overlay(loading, filtered, cx);
        let first_load = loading && export_rows_empty;
        let stale = loading && !export_rows_empty;
        let body = v_flex()
            .size_full()
            .child(toolbar)
            .child(motion::glimmer_bar(
                SharedString::from(format!("{id}-glimmer")),
                loading,
                cx,
            ))
            .child(
                div()
                    .id(SharedString::from(format!("{id}-grid")))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .when(stale, |d| d.opacity(0.55))
                    .child(render_row_table(&self.table, cx))
                    .when(first_load, |d| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .child(motion::skeleton_table("dv-skel", 10, 4, cx)),
                        )
                    })
                    .when_some(overlay, |d, o| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(cx.theme().background)
                                .child(o),
                        )
                    }),
            )
            .child(self.cell_detail.clone());

        let pager = sql_pagination_group(
            SharedString::from(format!("{id}-pager")),
            total,
            offset,
            page_size,
            loading,
            move |page, cx| {
                if let Some(ent) = panel.upgrade() {
                    ent.update(cx, |panel, cx| panel.go_to_page(page, cx));
                }
            },
            cx,
        );
        let read_only = cx
            .try_global::<WorkspaceRef>()
            .map(|ws| is_connection_read_only(&self.conn_id, ws.0.read(cx).registry().read(cx), cx))
            .unwrap_or(false);
        let crumbs = tab_breadcrumb_for_connection(&self.conn_id, self.source.path(), cx);
        let footer = tab_breadcrumb_footer(
            SharedString::from(format!("{id}-breadcrumb")),
            crumbs,
            Some(tab_breadcrumb_data_viewer_trailing(
                pager,
                self.last_load_ms,
                self.last_fetched
                    .map(|t| format_relative(t, OffsetDateTime::now_utc())),
                Button::new(SharedString::from(format!("{id}-refresh")))
                    .ghost()
                    .small()
                    .child(refresh_icon(loading))
                    .tooltip("Refresh")
                    .disabled(loading)
                    .on_click(cx.listener(|panel, _, _, cx| panel.load_page(panel.offset, cx))),
                read_only,
                "dv-read-only",
                cx,
            )),
            cx,
        );

        panel_tab_content(body, footer).into_any_element()
    }
}
