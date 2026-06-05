use anyhow::Result;
use sqlx::{AssertSqlSafe, Column as SqlxColumn, PgPool, Row, TypeInfo};

#[derive(Debug, Clone)]
pub struct QueryColumn {
    pub name: String,
    pub type_name: String,
}

pub async fn insert_row(
    pool: &PgPool,
    schema: &str,
    table: &str,
    values: &[(String, String)],
) -> Result<u64> {
    if values.is_empty() {
        anyhow::bail!("no columns");
    }
    let cols: Vec<_> = values.iter().map(|(c, _)| c.as_str()).collect();
    let placeholders: Vec<_> = (1..=values.len()).map(|i| format!("${i}")).collect();
    let sql = format!(
        r#"INSERT INTO "{schema}"."{table}" ({}) VALUES ({})"#,
        cols.iter()
            .map(|c| format!(r#""{c}""#))
            .collect::<Vec<_>>()
            .join(", "),
        placeholders.join(", ")
    );
    let mut q = sqlx::query(AssertSqlSafe(sql));
    for (_, v) in values {
        q = q.bind(v);
    }
    let r = q.execute(pool).await?;
    Ok(r.rows_affected())
}

pub async fn delete_row(
    pool: &PgPool,
    schema: &str,
    table: &str,
    pk_col: &str,
    pk_val: &str,
) -> Result<u64> {
    let sql = format!(r#"DELETE FROM "{schema}"."{table}" WHERE "{pk_col}" = $1"#);
    let r = sqlx::query(AssertSqlSafe(sql))
        .bind(pk_val)
        .execute(pool)
        .await?;
    Ok(r.rows_affected())
}

pub async fn execute_sql(
    pool: &PgPool,
    sql: &str,
) -> Result<(Vec<QueryColumn>, Vec<Vec<String>>, u64)> {
    let t = sql.trim_start();
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("select")
        || lower.starts_with("with")
        || lower.starts_with("explain")
        || lower.starts_with("show")
    {
        let rows = sqlx::query(AssertSqlSafe(sql)).fetch_all(pool).await?;
        let columns: Vec<QueryColumn> = rows
            .first()
            .map(|r| {
                r.columns()
                    .iter()
                    .map(|c| QueryColumn {
                        name: c.name().to_string(),
                        type_name: c.type_info().name().to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let data: Vec<Vec<String>> = rows
            .iter()
            .map(|row| {
                (0..row.len())
                    .map(|i| {
                        row.try_get::<String, _>(i)
                            .unwrap_or_else(|_| "".to_string())
                    })
                    .collect()
            })
            .collect();
        Ok((columns, data, 0))
    } else {
        let r = sqlx::query(AssertSqlSafe(sql)).execute(pool).await?;
        Ok((vec![], vec![], r.rows_affected()))
    }
}
