//! SQLx embedded migrations for the metadata SQLite database.

use anyhow::{Context, Result};
use sqlx::SqlitePool;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &SqlitePool) -> Result<()> {
    MIGRATOR
        .run(pool)
        .await
        .context("run metadata migrations")?;
    Ok(())
}
