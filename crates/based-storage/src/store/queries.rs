//! Collections and saved queries.

use anyhow::{Context, Result, bail};
use based_workspace::{Collection, LooseQuery, SavedQueryRef};
use uuid::Uuid;

use super::{MetadataStore, parse_uuid};

impl MetadataStore {
    // ── Collections & queries ───────────────────────────────────────────────

    pub async fn create_collection(&self, workspace_id: Uuid, name: &str) -> Result<Collection> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM collections WHERE workspace_id = ?")
                .bind(workspace_id.to_string())
                .fetch_one(&self.pool)
                .await
                .context("count collections")?;
        let collection = Collection {
            id: Uuid::new_v4(),
            name: name.to_string(),
            queries: Vec::new(),
        };
        sqlx::query(
            "INSERT INTO collections (id, workspace_id, name, sort_order)
             VALUES (?, ?, ?, ?)",
        )
        .bind(collection.id.to_string())
        .bind(workspace_id.to_string())
        .bind(&collection.name)
        .bind(count)
        .execute(&self.pool)
        .await
        .context("insert collection")?;
        self.touch_workspace(workspace_id).await?;
        Ok(collection)
    }

    pub async fn create_loose_query(
        &self,
        workspace_id: Uuid,
        name: &str,
        sql: &str,
        connection_template_id: Option<Uuid>,
    ) -> Result<LooseQuery> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM queries WHERE workspace_id = ? AND collection_id IS NULL",
        )
        .bind(workspace_id.to_string())
        .fetch_one(&self.pool)
        .await
        .context("count loose queries")?;
        let query = LooseQuery {
            id: Uuid::new_v4(),
            name: name.to_string(),
            sql: sql.to_string(),
            connection_template_id,
        };
        sqlx::query(
            "INSERT INTO queries
             (id, workspace_id, collection_id, name, sql_text, connection_template_id, sort_order)
             VALUES (?, ?, NULL, ?, ?, ?, ?)",
        )
        .bind(query.id.to_string())
        .bind(workspace_id.to_string())
        .bind(&query.name)
        .bind(&query.sql)
        .bind(connection_template_id.map(|u| u.to_string()))
        .bind(count)
        .execute(&self.pool)
        .await
        .context("insert loose query")?;
        self.touch_workspace(workspace_id).await?;
        Ok(query)
    }

    pub async fn move_query_to_collection(
        &self,
        workspace_id: Uuid,
        query_id: Uuid,
        collection_id: Uuid,
    ) -> Result<()> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM queries WHERE collection_id = ?")
            .bind(collection_id.to_string())
            .fetch_one(&self.pool)
            .await
            .context("count collection queries")?;
        let rows = sqlx::query(
            "UPDATE queries SET collection_id = ?, sort_order = ?
             WHERE id = ? AND workspace_id = ? AND collection_id IS NULL",
        )
        .bind(collection_id.to_string())
        .bind(count)
        .bind(query_id.to_string())
        .bind(workspace_id.to_string())
        .execute(&self.pool)
        .await
        .context("move query to collection")?
        .rows_affected();
        if rows == 0 {
            bail!("loose query not found");
        }
        self.touch_workspace(workspace_id).await?;
        Ok(())
    }

    pub async fn move_query_to_loose(&self, workspace_id: Uuid, query_id: Uuid) -> Result<()> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM queries WHERE workspace_id = ? AND collection_id IS NULL",
        )
        .bind(workspace_id.to_string())
        .fetch_one(&self.pool)
        .await
        .context("count loose queries")?;
        let rows = sqlx::query(
            "UPDATE queries SET collection_id = NULL, sort_order = ?
             WHERE id = ? AND workspace_id = ? AND collection_id IS NOT NULL",
        )
        .bind(count)
        .bind(query_id.to_string())
        .bind(workspace_id.to_string())
        .execute(&self.pool)
        .await
        .context("move query to loose")?
        .rows_affected();
        if rows == 0 {
            bail!("collection query not found");
        }
        self.touch_workspace(workspace_id).await?;
        Ok(())
    }

    pub(super) async fn load_collections(&self, workspace_id: &str) -> Result<Vec<Collection>> {
        let coll_rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, name FROM collections WHERE workspace_id = ? ORDER BY sort_order, name",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .context("load collections")?;

        let mut out = Vec::new();
        for (coll_id, name) in coll_rows {
            let queries = self.load_collection_queries(&coll_id).await?;
            out.push(Collection {
                id: parse_uuid(&coll_id)?,
                name,
                queries,
            });
        }
        Ok(out)
    }

    pub(super) async fn load_collection_queries(
        &self,
        collection_id: &str,
    ) -> Result<Vec<SavedQueryRef>> {
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, sql_text, connection_template_id
             FROM queries WHERE collection_id = ? ORDER BY sort_order, name",
        )
        .bind(collection_id)
        .fetch_all(&self.pool)
        .await
        .context("load collection queries")?;

        rows.into_iter()
            .map(|(id, name, sql, template_id)| {
                Ok(SavedQueryRef {
                    id: parse_uuid(&id)?,
                    name,
                    sql,
                    connection_template_id: template_id.map(|s| parse_uuid(&s)).transpose()?,
                })
            })
            .collect()
    }

    pub(super) async fn load_loose_queries(&self, workspace_id: &str) -> Result<Vec<LooseQuery>> {
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, name, sql_text, connection_template_id
             FROM queries
             WHERE workspace_id = ? AND collection_id IS NULL
             ORDER BY sort_order, name",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .context("load loose queries")?;

        rows.into_iter()
            .map(|(id, name, sql, template_id)| {
                Ok(LooseQuery {
                    id: parse_uuid(&id)?,
                    name,
                    sql,
                    connection_template_id: template_id.map(|s| parse_uuid(&s)).transpose()?,
                })
            })
            .collect()
    }
}
