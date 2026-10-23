// Paginated table viewer shared by SQL engines; engines plug in via [`DataSource`].

use gpui_kit::assets::IconName as AssetIcon;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use futures::future::BoxFuture;
use gpui_kit::component::table::TableEvent;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable as _,
    button::Button,
    dock::{BasePanel, Panel, PanelEvent},
    menu::{PopupMenu, PopupMenuItem},
    table::{Column, TableState},
    v_flex,
};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;
use crate::connection::ConnectionId;
use crate::connection::is_connection_read_only;
use crate::db;
use crate::widgets::cell_detail::{CellDetail, CellValue, interpret_cell_with_meta};
use crate::widgets::column_header::ColumnMeta;
use crate::widgets::data_table::{configure_row_table, render_row_table};
use crate::widgets::export_popover::export_popover;
use crate::widgets::filter_bar::{FilterBar, FilterExpr};
use crate::widgets::pagination::{offset_for_page, sql_pagination_group};
use crate::widgets::panel::{
    data_viewer_toolbar, panel_tab_content, tab_breadcrumb_data_viewer_trailing,
    tab_breadcrumb_footer, tab_breadcrumb_for_connection,
};
use crate::widgets::virtual_table::{
    NULL_CELL_DISPLAY, RowDelegate, align_meta_to_columns, data_column, replace_table_data,
};
use crate::workspace::WorkspaceRef;
use crate::workspace::pop_out::PopOutWindowTitle;
use crate::workspace::tabs::{DrillFilter, TabSpec, enqueue_open_tab, request_workspace_flush};

pub type Fut<T> = BoxFuture<'static, anyhow::Result<T>>;

/// One page of table rows, already formatted for display.
pub struct PageData {
    pub total: i64,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<SharedString>>,
}

impl PageData {
    /// `names` are the result-set column names; when the page is empty, fall back to the catalog.
    pub fn new(
        total: i64,
        names: Option<Vec<String>>,
        rows: Vec<Vec<SharedString>>,
        catalog: &HashMap<String, ColumnMeta>,
    ) -> Self {
        let names = names.unwrap_or_else(|| {
            let mut keys: Vec<_> = catalog.keys().cloned().collect();
            keys.sort();
            keys
        });
        let columns = names
            .into_iter()
            .map(|n| data_column(n.clone(), n))
            .collect();
        Self {
            total,
            columns,
            rows,
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
    fn catalog(&self) -> Fut<HashMap<String, ColumnMeta>>;
    fn page(
        &self,
        filter: Option<FilterExpr>,
        offset: u64,
        limit: u64,
        catalog: HashMap<String, ColumnMeta>,
    ) -> Fut<PageData>;
}

pub struct DataViewerPanel {
    focus_handle: FocusHandle,
    source: Arc<dyn DataSource>,
    conn_id: ConnectionId,
    table: Entity<TableState<RowDelegate>>,
    cell_detail: Entity<CellDetail>,
    filter_bar: Entity<FilterBar>,
    offset: u64,
    page_size: u64,
    total_rows: u64,
    loading: bool,
    last_load_ms: Option<u64>,
    column_catalog: HashMap<String, ColumnMeta>,
    /// Foreign-key jump for the selected cell.
    fk_jump: Option<(String, DrillFilter)>,
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
        let cell_detail = cx.new(|_| CellDetail::new());
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
            column_catalog: HashMap::new(),
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
        panel.table.update(cx, |t, _| {
            t.delegate_mut().cell_menu =
                Some(Rc::new(move |menu, row, col, cx| match weak.upgrade() {
                    Some(p) => p.read(cx).cell_menu(menu, row, col, cx),
                    None => menu,
                }));
        });
        panel.load_page(0, cx);
        panel
    }

    fn cell_snapshot(&self, row: usize, col: usize, cx: &App) -> Option<(String, CellValue)> {
        let st = self.table.read(cx);
        let del = st.delegate();
        let col_name = del.columns.get(col)?.key.to_string();
        let txt = del.rows.get(row)?.get(col)?.to_string();
        let meta = del.column_meta.get(col).cloned().unwrap_or_default();
        Some((col_name, interpret_cell_with_meta(&txt, &meta)))
    }

    /// Resolve the FK target (`[schema.]table(col)`) of a cell into a drill-down.
    fn fk_target_for(&self, row: usize, col: usize, cx: &App) -> Option<(String, DrillFilter)> {
        let del = self.table.read(cx).delegate();
        let target = del.column_meta.get(col)?.fk_target.as_ref()?;
        let value = del.rows.get(row)?.get(col)?.to_string();
        if value.is_empty() || value == NULL_CELL_DISPLAY {
            return None;
        }
        let (table, column) = target.strip_suffix(')')?.rsplit_once('(')?;
        let filter = DrillFilter {
            column: column.to_string(),
            value,
        };
        Some((table.to_string(), filter))
    }

    /// Right-click menu for the cell at `(row, col)`.
    fn cell_menu(&self, menu: PopupMenu, row: usize, col: usize, cx: &App) -> PopupMenu {
        let del = self.table.read(cx).delegate();
        let (Some(name), Some(value)) = (
            del.columns.get(col).map(|c| c.key.to_string()),
            del.rows
                .get(row)
                .and_then(|r| r.get(col))
                .map(|v| v.to_string()),
        ) else {
            return menu;
        };
        let row_text = del.rows[row]
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>();
        let headers: Vec<String> = del.columns.iter().map(|c| c.key.to_string()).collect();
        let row_tsv = row_text.join("\t");
        let row_json =
            serde_json::to_string_pretty(&headers.iter().zip(&row_text).collect::<HashMap<_, _>>())
                .unwrap_or_default();
        let copy = |label: &'static str, text: String| {
            PopupMenuItem::new(label).on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
            })
        };
        let this = self.self_handle.clone();
        let (fv, fk) = (value.clone(), self.fk_target_for(row, col, cx));
        let mut menu = menu
            .item(copy("Copy Value", value))
            .item(copy("Copy Row (TSV)", row_tsv))
            .item(copy("Copy Row (JSON)", row_json))
            .separator()
            .item(PopupMenuItem::new("Filter by Value").on_click({
                let this = this.clone();
                move |_, window, cx| {
                    let (name, fv) = (name.clone(), fv.clone());
                    let _ = this.update(cx, |p, cx| {
                        p.filter_bar
                            .update(cx, |fb, cx| fb.preset_eq(name, &fv, window, cx));
                        p.load_page(0, cx);
                    });
                }
            }));
        if let Some((table, f)) = fk {
            menu = menu.item(
                PopupMenuItem::new(format!("Go to {table} · {}={}", f.column, f.value)).on_click(
                    move |_, _, cx| {
                        let _ = this.update(cx, |p, cx| {
                            p.fk_jump = Some((table.clone(), f.clone()));
                            p.jump_to_fk(cx);
                        });
                    },
                ),
            );
        }
        menu
    }

    fn jump_to_fk(&mut self, cx: &mut Context<Self>) {
        let Some((object, filter)) = self.fk_jump.clone() else {
            return;
        };
        enqueue_open_tab(
            TabSpec::DataViewer {
                conn_id: self.conn_id.clone(),
                object,
                filter: Some(filter),
            },
            cx,
        );
        request_workspace_flush(cx);
    }

    fn load_page(&mut self, offset: u64, cx: &mut Context<Self>) {
        self.loading = true;
        self.offset = offset;
        let source = self.source.clone();
        let page_size = self.page_size;
        let filter = self.filter_bar.read(cx).current_expr(cx);
        let cached = self.column_catalog.clone();

        cx.spawn(async move |this, cx| {
            let catalog = if cached.is_empty() {
                db::run(cx, source.catalog()).await.unwrap_or_default()
            } else {
                cached
            };
            let fetch = source.page(filter, offset, page_size, catalog.clone());
            let res = db::run(cx, async move {
                let start = Instant::now();
                let page = fetch.await?;
                Ok((page, start.elapsed().as_millis() as u64))
            })
            .await;

            let _ = this.update(cx, |panel, cx| {
                panel.loading = false;
                if let Ok((page, elapsed_ms)) = res {
                    panel.total_rows = page.total as u64;
                    panel.last_load_ms = Some(elapsed_ms);
                    let names: Vec<String> =
                        page.columns.iter().map(|c| c.key.to_string()).collect();
                    let meta = align_meta_to_columns(names.iter().cloned(), &catalog);
                    panel.column_catalog = catalog;
                    panel.filter_bar.update(cx, |fb, cx| {
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

    fn go_to_page(&mut self, page_1_based: usize, cx: &mut Context<Self>) {
        self.load_page(offset_for_page(page_1_based, self.page_size), cx);
    }
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
        let muted = cx.theme().muted_foreground;

        let (export_headers, export_rows) = {
            let st = self.table.read(cx);
            let d = st.delegate();
            let h = d.columns.iter().map(|c| c.key.to_string()).collect();
            let r = d
                .rows
                .iter()
                .map(|row| row.iter().map(|c| c.to_string()).collect())
                .collect();
            (h, r)
        };

        let toolbar = data_viewer_toolbar(cx)
            .child(self.filter_bar.clone())
            .child(
                Button::new(SharedString::from(format!("{id}-filter-apply")))
                    .small()
                    .outline()
                    .icon(Icon::new(AssetIcon::Funnel))
                    .label("Apply filter")
                    .on_click(cx.listener(|panel, _, _, cx| panel.load_page(0, cx))),
            )
            .child(
                Button::new(SharedString::from(format!("{id}-filter-clear")))
                    .small()
                    .outline()
                    .icon(Icon::new(AssetIcon::FunnelX))
                    .tooltip("Clear filter")
                    .on_click(cx.listener(|panel, _, window, cx| {
                        panel.filter_bar.update(cx, |fb, cx| fb.clear(window, cx));
                        panel.load_page(0, cx);
                    })),
            )
            .when_some(self.fk_jump.clone(), |d, (table, f)| {
                d.child(
                    Button::new(SharedString::from(format!("{id}-fk-jump")))
                        .small()
                        .outline()
                        .icon(Icon::new(AssetIcon::ExternalLink))
                        .label(format!("{table} · {}={}", f.column, f.value))
                        .tooltip("Go to referenced row")
                        .on_click(cx.listener(|panel, _, _, cx| panel.jump_to_fk(cx))),
                )
            })
            .child(div().flex_1())
            .when(loading, |d| {
                d.child(div().text_sm().text_color(muted).child("Loading…"))
            })
            .child(export_popover(id, export_headers, export_rows));

        let body = v_flex()
            .size_full()
            .child(toolbar)
            .child(
                div()
                    .id(SharedString::from(format!("{id}-grid")))
                    .flex_1()
                    .min_h_0()
                    .child(render_row_table(&self.table, cx)),
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
                read_only,
                "dv-read-only",
                cx,
            )),
            cx,
        );

        panel_tab_content(body, footer).into_any_element()
    }
}
