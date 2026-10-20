//! Workspace connection templates.

use anyhow::{Context, Result};
use based_workspace::ConnectionTemplate;
use uuid::Uuid;

use super::{MetadataStore, parse_uuid};
use crate::secrets::SecretStore;

impl MetadataStore {
    // ── Connection templates ────────────────────────────────────────────────

    pub async fn upsert_connection_template(
        &self,
        workspace_id: Uuid,
        template: &ConnectionTemplate,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.context("begin upsert template")?;
        let ws_id = workspace_id.to_string();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM connection_templates WHERE workspace_id = ?")
                .bind(&ws_id)
                .fetch_one(&mut *tx)
                .await
                .context("count templates")?;
        self.insert_template(&mut tx, &ws_id, template, count)
            .await?;
        tx.commit().await.context("commit upsert template")?;
        self.touch_workspace(workspace_id).await?;
        Ok(())
    }

    pub async fn set_template_password_secret(
        &self,
        template_id: Uuid,
        password: &str,
    ) -> Result<()> {
        let ref_key = SecretStore::template_password_ref(template_id);
        self.secrets.set(&ref_key, password)?;
        sqlx::query(
            "UPDATE connection_templates SET password_secret_ref = ?, password_template = ''
             WHERE id = ?",
        )
        .bind(&ref_key)
        .bind(template_id.to_string())
        .execute(&self.pool)
        .await
        .context("record template secret ref")?;
        Ok(())
    }

    pub(super) async fn load_templates(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ConnectionTemplate>> {
        let rows: Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
        )> = sqlx::query_as(
            "SELECT id, label, engine, host, port, database_name, username, password_template,
                    password_secret_ref, ssl_mode
             FROM connection_templates
             WHERE workspace_id = ?
             ORDER BY sort_order, label",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .context("load templates")?;

        let mut out = Vec::new();
        for (
            id,
            label,
            engine,
            host,
            port,
            database,
            username,
            password_template,
            password_secret_ref,
            ssl_mode,
        ) in rows
        {
            let id = parse_uuid(&id)?;
            let password = if let Some(ref_key) = password_secret_ref {
                self.secrets.get(&ref_key)?.unwrap_or_default()
            } else {
                password_template
            };
            out.push(ConnectionTemplate {
                id,
                label,
                engine: based_core::EngineKind::from_str_lossy(&engine),
                host,
                port,
                database,
                username,
                password,
                ssl_mode,
            });
        }
        Ok(out)
    }

    pub(super) async fn insert_template(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        workspace_id: &str,
        template: &ConnectionTemplate,
        sort_order: i64,
    ) -> Result<()> {
        let (password_template, password_secret_ref) =
            split_template_password(template.id, &template.password, &self.secrets)?;

        sqlx::query(
            "INSERT INTO connection_templates
             (id, workspace_id, label, engine, host, port, database_name, username,
              password_template, password_secret_ref, ssl_mode, sort_order)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               label = excluded.label,
               engine = excluded.engine,
               host = excluded.host,
               port = excluded.port,
               database_name = excluded.database_name,
               username = excluded.username,
               password_template = excluded.password_template,
               password_secret_ref = excluded.password_secret_ref,
               ssl_mode = excluded.ssl_mode,
               sort_order = excluded.sort_order",
        )
        .bind(template.id.to_string())
        .bind(workspace_id)
        .bind(&template.label)
        .bind(template.engine.as_str())
        .bind(&template.host)
        .bind(&template.port)
        .bind(&template.database)
        .bind(&template.username)
        .bind(password_template)
        .bind(password_secret_ref)
        .bind(&template.ssl_mode)
        .bind(sort_order)
        .execute(&mut **tx)
        .await
        .context("insert connection template")?;
        Ok(())
    }
}

fn split_template_password(
    template_id: Uuid,
    password: &str,
    secrets: &SecretStore,
) -> Result<(String, Option<String>)> {
    if password.is_empty() {
        return Ok((String::new(), None));
    }
    if password.contains("{{") {
        return Ok((password.to_string(), None));
    }
    let ref_key = SecretStore::template_password_ref(template_id);
    secrets.set(&ref_key, password)?;
    Ok((String::new(), Some(ref_key)))
}
