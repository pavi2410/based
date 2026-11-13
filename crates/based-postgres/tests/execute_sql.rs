//! Runs against a live Postgres. Set `BASED_TEST_PG_URL` (e.g. the docker compose
//! instance: `postgres://based:based@localhost:15432/based`); skipped otherwise.

use std::env;

use based_core::{ResultSet, SqlOutcome, Value};
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

async fn rows(pool: &PgPool, sql: &str) -> ResultSet {
    match execute_sql(pool, sql).await.unwrap() {
        SqlOutcome::Rows(set) => set,
        other => panic!("{sql}: expected rows, got {other:?}"),
    }
}

fn names(set: &ResultSet) -> Vec<&str> {
    set.column_names().collect()
}

#[tokio::test]
async fn row_returning_statements_return_rows() {
    let Some(pool) = pool().await else { return };

    let set = rows(&pool, "-- Active users\nSELECT 1 AS one").await;
    assert_eq!(names(&set), ["one"]);
    assert_eq!(set.rows, [[Value::Int(1)]]);

    let set = rows(&pool, "/* c */ (SELECT 2 AS two)").await;
    assert_eq!(names(&set), ["two"]);
    assert_eq!(set.rows.len(), 1);

    let set = rows(&pool, "VALUES (1), (2)").await;
    assert_eq!(set.columns.len(), 1);
    assert_eq!(set.rows.len(), 2);
}

#[tokio::test]
async fn null_and_the_text_null_stay_distinct() {
    let Some(pool) = pool().await else { return };
    let set = rows(&pool, "SELECT NULL::text, 'NULL'::text, ''::text").await;
    assert_eq!(
        set.rows,
        [[Value::Null, Value::text("NULL"), Value::text("")]]
    );
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

    let set = rows(&pool, "SELECT id, name FROM based_exec_test").await;
    assert_eq!(names(&set), ["id", "name"]);
    assert!(set.rows.is_empty());

    assert_eq!(
        execute_sql(
            &pool,
            "INSERT INTO based_exec_test VALUES (1, 'a'), (2, 'b')"
        )
        .await
        .unwrap(),
        SqlOutcome::Affected(2)
    );

    let set = rows(
        &pool,
        "INSERT INTO based_exec_test VALUES (3, 'c') RETURNING id",
    )
    .await;
    assert_eq!(names(&set), ["id"]);
    assert_eq!(set.rows, [[Value::Int(3)]]);

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
    let set = rows(&pool, "SELECT * FROM based_tmp").await;
    assert_eq!(names(&set), ["id", "note"]);
    assert!(set.rows.is_empty());
}
