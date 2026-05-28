use anyhow::Result;
use sqlx::{Column as SqlxColumn, Row, SqlitePool};

pub async fn insert_row(
    pool: &SqlitePool,
    table: &str,
    values: &[(String, String)],
) -> Result<i64> {
    let cols: Vec<&str> = values.iter().map(|(k, _)| k.as_str()).collect();
    let placeholders: Vec<String> = (1..=values.len()).map(|i| format!("?{i}")).collect();

    let sql = format!(
        "INSERT INTO \"{table}\" ({}) VALUES ({})",
        cols.join(", "),
        placeholders.join(", ")
    );

    let mut q = sqlx::query(&sql);
    for (_, v) in values {
        q = q.bind(v);
    }
    let result = q.execute(pool).await?;
    Ok(result.last_insert_rowid())
}

pub async fn update_row(
    pool: &SqlitePool,
    table: &str,
    pk_col: &str,
    pk_val: &str,
    changes: &[(String, String)],
) -> Result<u64> {
    let set_clauses: Vec<String> = changes
        .iter()
        .enumerate()
        .map(|(i, (col, _))| format!("\"{col}\" = ?{}", i + 1))
        .collect();

    let pk_bind_idx = changes.len() + 1;
    let sql = format!(
        "UPDATE \"{table}\" SET {} WHERE \"{pk_col}\" = ?{pk_bind_idx}",
        set_clauses.join(", ")
    );

    let mut q = sqlx::query(&sql);
    for (_, v) in changes {
        q = q.bind(v);
    }
    q = q.bind(pk_val);
    let result = q.execute(pool).await?;
    Ok(result.rows_affected())
}

pub async fn delete_row(pool: &SqlitePool, table: &str, pk_col: &str, pk_val: &str) -> Result<u64> {
    let sql = format!("DELETE FROM \"{table}\" WHERE \"{pk_col}\" = ?1");
    let result = sqlx::query(&sql).bind(pk_val).execute(pool).await?;
    Ok(result.rows_affected())
}

/// Execute arbitrary SQL.
/// Returns `(column_names, rows, rows_affected)`.
pub async fn execute_sql(
    pool: &SqlitePool,
    sql: &str,
) -> Result<(Vec<String>, Vec<Vec<String>>, u64)> {
    let trimmed = sql.trim_start().to_ascii_uppercase();
    if trimmed.starts_with("SELECT")
        || trimmed.starts_with("WITH")
        || trimmed.starts_with("EXPLAIN")
    {
        let rows = sqlx::query(sql).fetch_all(pool).await?;
        let columns: Vec<String> = if let Some(first) = rows.first() {
            first
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect()
        } else {
            vec![]
        };
        let data: Vec<Vec<String>> = rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|i| row.try_get::<String, _>(i).unwrap_or_default())
                    .collect()
            })
            .collect();
        Ok((columns, data, 0))
    } else {
        let result = sqlx::query(sql).execute(pool).await?;
        Ok((vec![], vec![], result.rows_affected()))
    }
}
