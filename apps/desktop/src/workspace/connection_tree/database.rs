//! Database switcher for Postgres (named databases) and Redis (numbered DBs).

use std::mem;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, WeakEntity, div, prelude::*, px,
};

use crate::connection::{
    AnyConnection, ConnectionConfig, ConnectionId, ConnectionState, EngineKind,
    close_any_connection, open_connection,
};
use crate::db::run;
use crate::widgets::SIDEBAR_INSET;

use super::ConnectionTree;
use super::connect::finish_connection_open;

/// Name of the database a config currently points at.
fn current_database(config: &ConnectionConfig) -> Option<String> {
    match config {
        ConnectionConfig::Postgres(c) => Some(c.database.clone()),
        ConnectionConfig::Redis(c) => Some(based_redis::url_db(&c.url).to_string()),
        _ => None,
    }
}

fn display_name(engine: EngineKind, db: &str) -> String {
    match engine {
        EngineKind::Redis => format!("db{db}"),
        _ => db.to_string(),
    }
}

impl ConnectionTree {
    /// Fetch the databases available on a freshly connected Postgres / Redis server.
    pub(crate) fn load_databases(
        &mut self,
        conn_id: ConnectionId,
        ac: &AnyConnection,
        cx: &mut Context<Self>,
    ) {
        enum Source {
            Pg(sqlx::PgPool),
            Redis(based_redis::Conn),
        }
        let source = match ac {
            AnyConnection::Postgres(conn) => Source::Pg(conn.read(cx).pool.clone()),
            AnyConnection::Redis(conn) => Source::Redis(conn.read(cx).conn()),
            _ => return,
        };
        cx.spawn(async move |this, cx| {
            let result = run(cx, async move {
                Ok::<_, anyhow::Error>(match source {
                    Source::Pg(pool) => {
                        sqlx::query_scalar::<_, String>(
                            "SELECT datname FROM pg_database \
                             WHERE NOT datistemplate AND datallowconn ORDER BY datname",
                        )
                        .fetch_all(&pool)
                        .await?
                    }
                    Source::Redis(mut conn) => {
                        let n = based_redis::database_count(&mut conn).await;
                        (0..n).map(|i| i.to_string()).collect()
                    }
                })
            })
            .await;
            let Ok(dbs) = result else { return };
            let _ = this.update(cx, |tree, cx| {
                if let Some(st) = tree.conn_states.get_mut(&conn_id) {
                    st.databases = dbs;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Reconnect to another database; the saved profile on disk is left untouched.
    pub(crate) fn switch_database(&mut self, idx: usize, db: String, cx: &mut Context<Self>) {
        let Some(ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let mut config = ent.read(cx).config.clone();
        match &mut config {
            ConnectionConfig::Postgres(c) => c.database = db,
            ConnectionConfig::Redis(c) => {
                let Ok(n) = db.parse() else { return };
                c.url = based_redis::url_with_db(&c.url, n);
            }
            _ => return,
        }
        if current_database(&config) == current_database(&ent.read(cx).config) {
            return;
        }

        let conn_id = ent.read(cx).id.clone();
        let old = ent.update(cx, |e, cx| {
            e.config = config.clone();
            e.last_error = None;
            cx.notify();
            mem::replace(&mut e.state, ConnectionState::Connecting)
        });
        if let ConnectionState::Connected(ac) = old {
            close_any_connection(ac, cx);
        }
        if let Some(st) = self.conn_states.get_mut(&conn_id) {
            st.cache_mut().invalidate();
            st.expanded_schemas.clear();
        }
        self.selected_object = None;
        self.bump_object_list_epoch(cx);

        let label = config.label().to_string();
        let engine = config.engine();
        let task = open_connection(config, cx);
        let tree = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            cx.update(|app| finish_connection_open(result, ent, tree, idx, label, engine, app));
        })
        .detach();
    }
}

/// Dropdown shown above the catalog; `None` for engines without switchable databases.
pub(crate) fn database_selector(
    tree: &ConnectionTree,
    tree_entity: WeakEntity<ConnectionTree>,
    cx: &gpui_kit::App,
) -> Option<AnyElement> {
    let idx = tree.selected_connection?;
    let entry = tree.registry.read(cx).connections().get(idx)?.read(cx);
    let engine = entry.engine();
    let current = current_database(&entry.config)?;
    let databases = tree.conn_states.get(&entry.id)?.databases.clone();
    if databases.is_empty() || !matches!(entry.state, ConnectionState::Connected(_)) {
        return None;
    }

    let theme = cx.theme();
    let label: SharedString = display_name(engine, &current).into();
    let muted = theme.muted_foreground;
    Some(
        h_flex()
            .h(px(36.0))
            .px(px(SIDEBAR_INSET))
            .items_center()
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.sidebar_border)
            .child(
                Button::new("database-switcher")
                    .ghost()
                    .small()
                    .w_full()
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(IconName::Folder).text_color(muted))
                            .child(div().flex_1().min_w_0().text_sm().truncate().child(label))
                            .child(Icon::new(IconName::ChevronDown).xsmall().text_color(muted)),
                    )
                    .dropdown_menu(move |menu, _window, _cx| {
                        databases.iter().fold(menu, |menu, db| {
                            let tree = tree_entity.clone();
                            let value = db.clone();
                            menu.item(
                                PopupMenuItem::new(display_name(engine, db))
                                    .checked(*db == current)
                                    .on_click(move |_, _, cx| {
                                        if let Some(ent) = tree.upgrade() {
                                            let value = value.clone();
                                            ent.update(cx, |t, cx| {
                                                t.switch_database(idx, value, cx)
                                            });
                                        }
                                    }),
                            )
                        })
                    }),
            )
            .into_any_element(),
    )
}
