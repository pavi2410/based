use gpui::Context;
use mongodb::Database;
use sqlx::{PgPool, Row, SqlitePool};

use crate::connection::{AnyConnection, ConnectionId, EngineKind};

use super::ConnectionTree;
use super::types::{ActiveObjects, ObjectKind, SchemaObject};

impl ConnectionTree {
    pub(crate) fn load_objects_for_connection(
        &mut self,
        idx: usize,
        ac: AnyConnection,
        cx: &mut Context<Self>,
    ) {
        let Some(ent) = self.registry.read(cx).connections().get(idx).cloned() else {
            return;
        };
        let (label, engine, conn_id) = {
            let entry = ent.read(cx);
            (
                entry.config.label().to_string(),
                entry.config.engine(),
                entry.id.clone(),
            )
        };

        if let Some(st) = self.conn_states.get_mut(&conn_id) {
            st.loading = true;
            st.error = None;
        }
        if self.selected_connection == Some(idx) {
            self.active_objects = ActiveObjects::Loading {
                label: label.clone(),
                engine,
            };
        }
        self.bump_object_list_epoch(cx);

        match ac {
            AnyConnection::SQLite(conn) => {
                let pool = conn.read(cx).pool.clone();
                self.load_sqlite_objects(idx, conn_id, label, engine, pool, cx);
            }
            AnyConnection::Postgres(conn) => {
                let pool = conn.read(cx).pool.clone();
                self.load_postgres_objects(idx, conn_id, label, engine, pool, cx);
            }
            AnyConnection::MongoDB(conn) => {
                let db = conn.read(cx).database().clone();
                self.load_mongo_objects(idx, conn_id, label, engine, db, cx);
            }
        }
    }

    fn load_sqlite_objects(
        &mut self,
        idx: usize,
        conn_id: ConnectionId,
        label: String,
        engine: EngineKind,
        pool: SqlitePool,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = crate::db::run(cx, async move {
                let rows = sqlx::query(
                    "SELECT name, type FROM sqlite_master \
                     WHERE type IN ('table','view','trigger') \
                     ORDER BY type, name",
                )
                .fetch_all(&pool)
                .await?;

                let objects = rows
                    .iter()
                    .map(|row| {
                        let name: String = row.get("name");
                        let kind_str: String = row.get("type");
                        let kind = match kind_str.as_str() {
                            "view" => ObjectKind::View,
                            "trigger" => ObjectKind::Trigger,
                            _ => ObjectKind::Table,
                        };
                        SchemaObject {
                            name,
                            schema: None,
                            kind,
                        }
                    })
                    .collect::<Vec<_>>();
                Ok(objects)
            })
            .await;

            let _ = this.update(cx, |tree, cx| {
                tree.apply_schema_load_result(idx, &conn_id, label, engine, result, cx);
            });
        })
        .detach();
    }

    fn load_postgres_objects(
        &mut self,
        idx: usize,
        conn_id: ConnectionId,
        label: String,
        engine: EngineKind,
        pool: PgPool,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = crate::db::run(cx, async move {
                let rows = sqlx::query(
                    r"SELECT table_schema, table_name, table_type
                      FROM information_schema.tables
                      WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                      ORDER BY table_schema, table_type, table_name",
                )
                .fetch_all(&pool)
                .await?;

                let objects = rows
                    .iter()
                    .map(|row| {
                        let schema: String = row.get("table_schema");
                        let name: String = row.get("table_name");
                        let ty: String = row.get("table_type");
                        let kind = match ty.as_str() {
                            "VIEW" => ObjectKind::View,
                            "MATERIALIZED VIEW" => ObjectKind::MaterializedView,
                            _ => ObjectKind::Table,
                        };
                        SchemaObject {
                            name,
                            schema: Some(schema),
                            kind,
                        }
                    })
                    .collect::<Vec<_>>();
                Ok(objects)
            })
            .await;

            let _ = this.update(cx, |tree, cx| {
                tree.apply_schema_load_result(idx, &conn_id, label, engine, result, cx);
            });
        })
        .detach();
    }

    fn load_mongo_objects(
        &mut self,
        idx: usize,
        conn_id: ConnectionId,
        label: String,
        engine: EngineKind,
        db: Database,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = crate::db::run(cx, async move {
                let names = db.list_collection_names(None).await?;
                let objects = names
                    .into_iter()
                    .map(|name| SchemaObject {
                        name,
                        schema: None,
                        kind: ObjectKind::Collection,
                    })
                    .collect::<Vec<_>>();
                Ok(objects)
            })
            .await;

            let _ = this.update(cx, |tree, cx| {
                tree.apply_schema_load_result(idx, &conn_id, label, engine, result, cx);
            });
        })
        .detach();
    }

    fn apply_schema_load_result(
        &mut self,
        idx: usize,
        conn_id: &ConnectionId,
        label: String,
        engine: EngineKind,
        result: Result<Vec<SchemaObject>, anyhow::Error>,
        cx: &mut Context<Self>,
    ) {
        if let Some(st) = self.conn_states.get_mut(conn_id) {
            st.loading = false;
            match &result {
                Ok(objects) => {
                    st.error = None;
                    st.objects = Some(objects.clone());
                }
                Err(err) => {
                    st.error = Some(err.to_string());
                    st.objects = None;
                }
            }
        }

        if self.selected_connection == Some(idx) {
            self.active_objects = match result {
                Ok(objects) => ActiveObjects::Ready {
                    label,
                    engine,
                    objects,
                },
                Err(err) => ActiveObjects::Error {
                    label,
                    message: err.to_string(),
                },
            };
        }

        self.bump_object_list_epoch(cx);
        cx.notify();
    }
}
