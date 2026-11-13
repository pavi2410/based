use gpui_kit::Context;
use mongodb::Database;
use sqlx::{PgPool, SqlitePool};

use crate::connection::{AnyConnection, ConnectionId, EngineKind};

use super::ConnectionTree;
use super::types::{DbObject, ObjectKind};
use crate::db::run;

/// Keys loaded into the sidebar per refresh (SCAN stops here).
const REDIS_KEY_LIMIT: usize = 2000;

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
        let (engine, conn_id) = {
            let entry = ent.read(cx);
            (entry.config.engine(), entry.id.clone())
        };

        if let Some(st) = self.conn_states.get_mut(&conn_id) {
            st.cache_mut().start_loading();
        }
        self.bump_object_list_epoch(cx);

        self.load_databases(conn_id.clone(), &ac, cx);

        match ac {
            AnyConnection::SQLite(conn) => {
                let pool = conn.read(cx).pool.clone();
                self.load_sqlite_objects(conn_id, engine, pool, cx);
            }
            AnyConnection::Postgres(conn) => {
                let pool = conn.read(cx).pool.clone();
                self.load_postgres_objects(conn_id, engine, pool, cx);
            }
            AnyConnection::MongoDB(conn) => {
                let db = conn.read(cx).database().clone();
                self.load_mongo_objects(conn_id, engine, db, cx);
            }
            AnyConnection::Redis(conn) => {
                let conn = conn.read(cx).conn();
                self.load_redis_keys(conn_id, engine, conn, cx);
            }
        }
    }

    fn load_sqlite_objects(
        &mut self,
        conn_id: ConnectionId,
        engine: EngineKind,
        pool: SqlitePool,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = run(cx, async move { based_sqlite::list_objects(&pool).await }).await;

            let _ = this.update(cx, |tree, cx| {
                tree.apply_schema_load_result(&conn_id, engine, result, cx);
            });
        })
        .detach();
    }

    fn load_postgres_objects(
        &mut self,
        conn_id: ConnectionId,
        engine: EngineKind,
        pool: PgPool,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = run(cx, async move { based_postgres::list_objects(&pool).await }).await;

            let _ = this.update(cx, |tree, cx| {
                tree.apply_schema_load_result(&conn_id, engine, result, cx);
            });
        })
        .detach();
    }

    fn load_mongo_objects(
        &mut self,
        conn_id: ConnectionId,
        engine: EngineKind,
        db: Database,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = run(cx, async move { based_mongo::list_collections(&db).await }).await;

            let _ = this.update(cx, |tree, cx| {
                tree.apply_schema_load_result(&conn_id, engine, result, cx);
            });
        })
        .detach();
    }

    fn load_redis_keys(
        &mut self,
        conn_id: ConnectionId,
        engine: EngineKind,
        mut conn: based_redis::Conn,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = run(cx, async move {
                let keys = based_redis::scan_keys(&mut conn, "*", REDIS_KEY_LIMIT).await?;
                Ok(keys
                    .into_iter()
                    .map(|name| DbObject {
                        name,
                        schema: None,
                        kind: ObjectKind::Key,
                    })
                    .collect::<Vec<_>>())
            })
            .await;

            let _ = this.update(cx, |tree, cx| {
                tree.apply_schema_load_result(&conn_id, engine, result, cx);
            });
        })
        .detach();
    }

    fn apply_schema_load_result(
        &mut self,
        conn_id: &ConnectionId,
        engine: EngineKind,
        result: Result<Vec<DbObject>, anyhow::Error>,
        cx: &mut Context<Self>,
    ) {
        if let Some(st) = self.conn_states.get_mut(conn_id) {
            match &result {
                Ok(objects) => {
                    st.cache_mut().set_ready(objects.clone());
                    if engine == EngineKind::Postgres {
                        st.seed_public_schema(objects);
                    }
                }
                Err(err) => st.cache_mut().set_error(err.to_string()),
            }
        }

        self.bump_object_list_epoch(cx);
        cx.notify();
    }
}
