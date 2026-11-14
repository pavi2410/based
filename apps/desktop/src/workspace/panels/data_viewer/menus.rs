//! Context menus on headers and cells, and the foreign-key drill-down they offer.

use super::*;

impl DataViewerPanel {
    pub(super) fn cell_snapshot(
        &self,
        row: usize,
        col: usize,
        cx: &App,
    ) -> Option<(String, Value)> {
        let st = self.table.read(cx);
        let del = st.delegate();
        let col_name = del.columns.get(col)?.key.to_string();
        let cell = del.rows.get(row)?.get(col)?;
        Some((col_name, cell.value.clone()))
    }

    /// The referenced table and `column = value` filter for a foreign-key cell.
    pub(super) fn fk_target_for(
        &self,
        row: usize,
        col: usize,
        cx: &App,
    ) -> Option<(ObjectRef, DrillFilter)> {
        let del = self.table.read(cx).delegate();
        let target = del.column_meta.get(col)?.fk.as_ref()?;
        let cell = del.rows.get(row)?.get(col)?;
        if cell.is_null() {
            return None;
        }
        let filter = DrillFilter {
            column: target.column.clone(),
            value: cell.display.to_string(),
        };
        Some((target.table.clone(), filter))
    }

    /// The "explain this column" question, or `None` when AI is off for the connection.
    /// Sample values are included only when the connection shares rows.
    pub(super) fn explain_prompt(&self, col: usize, owner: EntityId, cx: &App) -> Option<String> {
        let level = connection_ai(&self.conn_id, owner, cx);
        if level == ContextLevel::Off {
            return None;
        }
        let del = self.table.read(cx).delegate();
        let name = del.columns.get(col)?.key.to_string();
        let meta = del.column_meta.get(col).cloned().unwrap_or_default();
        let samples = if level == ContextLevel::SchemaRows {
            distinct_samples(&del.rows, col)
        } else {
            Vec::new()
        };
        Some(column_prompt(
            &self.source.path().join("."),
            &name,
            &meta,
            &samples,
        ))
    }

    /// Right-click menu for the header of column `col`.
    pub(super) fn header_menu(
        this: &Entity<Self>,
        menu: PopupMenu,
        col: usize,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let (table, filter_bar) = {
            let p = this.read(cx);
            (p.table.clone(), p.filter_bar.clone())
        };
        let Some(name) = table
            .read(cx)
            .delegate()
            .columns
            .get(col)
            .map(|c| c.name.clone())
        else {
            return menu;
        };
        let key = table.read(cx).delegate().columns[col].key.clone();
        let _ = window;
        let sort = |label: &'static str, icon: IconName, ascending: bool| {
            let table = table.clone();
            PopupMenuItem::new(label)
                .icon(icon)
                .on_click(move |_, window, cx| {
                    table.update(cx, |t, cx| sort_column(t, col, ascending, window, cx));
                })
        };
        let filter_name = name.clone();
        let copy_name = name.to_string();
        let hide_table = table.clone();
        let explain = this.read(cx).explain_prompt(col, this.entity_id(), cx);
        let conn = this.read(cx).conn_id.clone();
        let menu = match explain {
            Some(prompt) => menu.item(
                PopupMenuItem::new("Explain Column")
                    .icon(IconName::Bot)
                    .on_click(move |_, _, cx| ask_assistant(conn.clone(), prompt.clone(), cx)),
            ),
            None => menu,
        };
        menu.item(sort("Sort Ascending", IconName::ArrowUp, true))
            .item(sort("Sort Descending", IconName::ArrowDown, false))
            .separator()
            .item(
                PopupMenuItem::new("Filter by This Column")
                    .icon(Icon::new(AssetIcon::Funnel))
                    .on_click(move |_, window, cx| {
                        filter_bar.update(cx, |fb, cx| fb.start_term(&filter_name, window, cx));
                    }),
            )
            .item(
                PopupMenuItem::new("Copy Column Name")
                    .icon(Icon::empty().path("icons/copy.svg"))
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_name.clone()));
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new("Hide Column")
                    .icon(IconName::EyeOff)
                    .on_click(move |_, _, cx| {
                        hide_table.update(cx, |t, cx| toggle_column(t, &key, cx));
                    }),
            )
    }

    pub(super) fn cell_menu(
        p: &Entity<Self>,
        menu: PopupMenu,
        row: usize,
        col: usize,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        // Snapshot owned data so `cx` stays free for building the submenu.
        let snap = {
            let this = p.read(cx);
            let del = this.table.read(cx).delegate();
            let name = del.columns.get(col).map(|c| c.key.to_string());
            let value = del.rows.get(row).and_then(|r| r.get(col)).cloned();
            let row_text: Vec<String> = del
                .rows
                .get(row)
                .map(|r| r.iter().map(|c| c.display.to_string()).collect())
                .unwrap_or_default();
            let headers: Vec<String> = del.columns.iter().map(|c| c.key.to_string()).collect();
            let fk = this.fk_target_for(row, col, cx);
            let is_array = is_array_type(
                del.column_meta
                    .get(col)
                    .and_then(|m| m.data_type.as_deref()),
            );
            name.zip(value).map(|(n, v)| {
                (
                    n,
                    v,
                    row_text,
                    headers,
                    fk,
                    this.self_handle.clone(),
                    is_array,
                )
            })
        };
        let Some((name, value, row_text, headers, fk, this, is_array)) = snap else {
            return menu;
        };
        let row_tsv = row_text.join("\t");
        let row_json =
            serde_json::to_string_pretty(&headers.iter().zip(&row_text).collect::<HashMap<_, _>>())
                .unwrap_or_default();
        let copy = |label: &'static str, text: String| {
            PopupMenuItem::new(label)
                .icon(Icon::empty().path("icons/copy.svg"))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                })
        };
        let fv = value.clone();
        let mut menu = menu
            .item(copy("Copy Value", value.display.to_string()))
            .item(copy("Copy Row (TSV)", row_tsv))
            .item(copy("Copy Row (JSON)", row_json))
            .separator()
            .item(filter_item(
                "Filter by This Value",
                false,
                &name,
                &fv,
                &this,
            ))
            .item(filter_item("Exclude This Value", true, &name, &fv, &this))
            .submenu("More Filters", window, cx, {
                let this = this.clone();
                move |mut m, _, _| {
                    // `has` takes a single element; a whole-cell array can't supply one.
                    for op in FilterOp::for_column(is_array).filter(|o| *o != FilterOp::ArrayHas) {
                        let (this, name, fv) = (this.clone(), name.clone(), fv.clone());
                        let label = if op.has_value() {
                            format!("{name} {} value", op.label())
                        } else {
                            format!("{name} {}", op.label())
                        };
                        m = m.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                            let (name, fv) = (name.clone(), fv.clone());
                            let _ = this.update(cx, |p, cx| {
                                p.filter_bar.update(cx, |fb, cx| {
                                    fb.preset(name, op, fv.display.as_ref(), window, cx)
                                });
                                p.load_page(0, cx);
                            });
                        }));
                    }
                    m
                }
            });
        if let Some((table, f)) = fk {
            menu = menu.item(
                PopupMenuItem::new(format!("Go to {table} · {}={}", f.column, f.value))
                    .icon(Icon::new(AssetIcon::ExternalLink))
                    .on_click(move |_, _, cx| {
                        let _ = this.update(cx, |p, cx| {
                            p.fk_jump = Some((table.clone(), f.clone()));
                            p.jump_to_fk(cx);
                        });
                    }),
            );
        }
        menu
    }

    pub(super) fn jump_to_fk(&mut self, cx: &mut Context<Self>) {
        let Some((object, filter)) = self.fk_jump.clone() else {
            return;
        };
        inbox::open_tab(
            TabSpec::DataViewer {
                conn_id: self.conn_id.clone(),
                object,
                filter: Some(filter),
            },
            cx,
        );
        request_workspace_flush(cx);
    }
}
