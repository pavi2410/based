//! Saving the query to the project, and the run-history menu.

use super::*;

impl QueryEditorPanel {
    /// Where a saved copy of this query should run.
    pub(super) fn save_target(&self, cx: &Context<Self>) -> SaveTarget {
        let registry = Workspaces::registry_for_panel(cx.entity_id(), cx);
        save_target_for(&self.conn_id, self.source.engine(), registry.as_ref(), cx)
    }

    /// Save the editor text into the project: in place if it was saved before, else
    /// after asking for a name.
    pub(super) fn save_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = cx.try_global::<ProjectRoot>().map(|r| r.0.clone()) else {
            notify::push_info(cx, "Open a project to save queries.");
            return;
        };
        let sql = self.current_sql(cx);
        if sql.trim().is_empty() {
            return;
        }
        if let Some(saved) = self.saved.clone() {
            self.write_saved(&root, &saved.name, &sql, Some(&saved.path), cx);
            return;
        }
        let first_comment = sql
            .lines()
            .next()
            .and_then(|l| l.trim().strip_prefix("--"))
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());
        let name_input = new_field(
            window,
            cx,
            first_comment.as_deref().unwrap_or("Untitled query"),
            "Query name",
        );
        let panel = cx.entity().downgrade();
        let field = name_input.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let (panel, root, field) = (panel.clone(), root.clone(), field.clone());
            dialog
                .title("Save query")
                .child(Input::new(&name_input).aria_label("Query name").w_full())
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogClose::new()
                                .child(Button::new("save-query-cancel").outline().label("Cancel")),
                        )
                        .child(
                            DialogAction::new()
                                .child(Button::new("save-query-ok").primary().label("Save")),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    let name = field.read(cx).value().trim().to_string();
                    if name.is_empty() {
                        return false;
                    }
                    let (root, panel) = (root.clone(), panel.clone());
                    let _ = panel.update(cx, |p, cx| {
                        let sql = p.current_sql(cx);
                        p.write_saved(&root, &name, &sql, None, cx);
                    });
                    true
                })
        });
    }

    pub(super) fn write_saved(
        &mut self,
        root: &Path,
        name: &str,
        sql: &str,
        overwrite: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let target = self.save_target(cx);
        match save_sql_query(root, name, sql, &target, overwrite) {
            Ok(path) => {
                self.saved = Some(SavedQueryRef {
                    path,
                    name: name.to_string(),
                });
                self.tab_label = name.to_string().into();
                self.dirty = false;
                clear_tab_dirty(cx.entity_id(), cx);
                notify::push_info(cx, format!("Saved {name}"));
                cx.notify();
            }
            Err(e) => notify::push_error(cx, "Could not save query", e.to_string()),
        }
    }

    /// Past runs on this connection, newest first: (first line, age and time, full SQL, ok).
    pub(super) fn history_items(&self, cx: &App) -> Vec<(String, String, String, bool)> {
        let Some(store) = cx.try_global::<QueryStore>() else {
            return Vec::new();
        };
        let now = OffsetDateTime::now_utc();
        store
            .history_for(&self.conn_id)
            .into_iter()
            .take(15)
            .map(|h| {
                let line = h.query.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                let mut line = line.trim().to_string();
                if line.chars().count() > 56 {
                    line = line.chars().take(55).collect::<String>() + "…";
                }
                (
                    line,
                    format!(
                        "{} · {} ms",
                        format_relative(h.ran_at, now),
                        h.duration.as_millis()
                    ),
                    h.query.clone(),
                    h.outcome.is_ok(),
                )
            })
            .collect()
    }

    /// Toolbar menu that restores a past run into the editor.
    pub(super) fn history_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.source.id();
        let items = self.history_items(cx);
        let panel = cx.weak_entity();
        Button::new(format!("{id}-history"))
            .ghost()
            .small()
            .label("History")
            .dropdown_caret(true)
            .disabled(items.is_empty())
            .tooltip("Past runs on this connection")
            .dropdown_menu(move |menu, _, _| {
                items.iter().fold(menu, |menu, (line, meta, sql, ok)| {
                    let (panel, sql) = (panel.clone(), sql.clone());
                    menu.item(
                        PopupMenuItem::new(format!("{line}    {meta}"))
                            .icon(if *ok {
                                IconName::Check
                            } else {
                                IconName::TriangleAlert
                            })
                            .on_click(move |_, window, cx| {
                                let _ = panel.update(cx, |p, cx| p.set_sql(&sql, window, cx));
                            }),
                    )
                })
            })
    }
}

/// Where a query saved for `conn_id` runs: that connection when it is part of the project,
/// else any connection of its engine.
pub(crate) fn save_target_for(
    conn_id: &ConnectionId,
    engine: EngineKind,
    registry: Option<&Entity<ConnectionRegistry>>,
    cx: &App,
) -> SaveTarget {
    let entry = registry.and_then(|r| r.read(cx).get(conn_id, cx).cloned());
    if let Some(entry) = entry {
        let e = entry.read(cx);
        if let Some(key) = e.id.project_key() {
            return SaveTarget::Connection(key);
        }
    }
    SaveTarget::Engine(engine)
}
