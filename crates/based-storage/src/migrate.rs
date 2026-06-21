//! SQLx embedded migrations for the metadata SQLite database.

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &SqlitePool) -> Result<()> {
    MIGRATOR
        .run(pool)
        .await
        .context("run metadata migrations")?;
    Ok(())
}
