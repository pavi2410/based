//! Runs against a live Postgres. Set `BASED_TEST_PG_URL` (e.g. the docker compose
//! instance: `postgres://based:based@localhost:15432/based`); skipped otherwise.

use std::env;

use based_core::Value;
use based_postgres::execute_sql;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

async fn pool() -> Option<PgPool> {
    let url = env::var("BASED_TEST_PG_URL").ok()?;
    Some(
        PgPool::connect(&url)
            .await
            .expect("connect BASED_TEST_PG_URL"),
    )
}

fn names(cols: &[based_postgres::QueryColumn]) -> Vec<&str> {
    cols.iter().map(|c| c.name.as_str()).collect()
}

#[tokio::test]
async fn row_returning_statements_return_rows() {
    let Some(pool) = pool().await else { return };

    let (cols, rows, _) = execute_sql(&pool, "-- Active users\nSELECT 1 AS one")
        .await
        .unwrap();
    assert_eq!(names(&cols), ["one"]);
    assert_eq!(rows, [[Value::Int(1)]]);

    let (cols, rows, _) = execute_sql(&pool, "/* c */ (SELECT 2 AS two)")
        .await
        .unwrap();
    assert_eq!(names(&cols), ["two"]);
    assert_eq!(rows.len(), 1);

    let (cols, rows, _) = execute_sql(&pool, "VALUES (1), (2)").await.unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn null_and_the_text_null_stay_distinct() {
    let Some(pool) = pool().await else { return };
    let (_, rows, _) = execute_sql(&pool, "SELECT NULL::text, 'NULL'::text, ''::text")
        .await
        .unwrap();
    assert_eq!(rows, [[Value::Null, Value::text("NULL"), Value::text("")]]);
}

#[tokio::test]
async fn empty_select_keeps_columns_and_dml_reports_affected() {
    let Some(pool) = pool().await else { return };
    execute_sql(&pool, "DROP TABLE IF EXISTS based_exec_test")
        .await
        .unwrap();
    execute_sql(&pool, "CREATE TABLE based_exec_test (id int, name text)")
        .await
        .unwrap();

    let (cols, rows, _) = execute_sql(&pool, "SELECT id, name FROM based_exec_test")
        .await
        .unwrap();
    assert_eq!(names(&cols), ["id", "name"]);
    assert!(rows.is_empty());

    let (cols, rows, affected) = execute_sql(
        &pool,
        "INSERT INTO based_exec_test VALUES (1, 'a'), (2, 'b')",
    )
    .await
    .unwrap();
    assert!(cols.is_empty());
    assert!(rows.is_empty());
    assert_eq!(affected, 2);

    let (cols, rows, _) = execute_sql(
        &pool,
        "INSERT INTO based_exec_test VALUES (3, 'c') RETURNING id",
    )
    .await
    .unwrap();
    assert_eq!(names(&cols), ["id"]);
    assert_eq!(rows, [[Value::Int(3)]]);

    execute_sql(&pool, "DROP TABLE based_exec_test")
        .await
        .unwrap();
}

#[tokio::test]
async fn empty_select_from_a_temp_table_keeps_its_columns() {
    let Ok(url) = env::var("BASED_TEST_PG_URL") else {
        return;
    };
    // Temp tables live on one session; the column lookup must use the same one.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    execute_sql(&pool, "CREATE TEMP TABLE based_tmp (id int, note text)")
        .await
        .unwrap();
    let (cols, rows, _) = execute_sql(&pool, "SELECT * FROM based_tmp").await.unwrap();
    assert_eq!(names(&cols), ["id", "note"]);
    assert!(rows.is_empty());
}
