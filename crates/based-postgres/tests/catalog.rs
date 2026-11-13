//! Runs against a live Postgres. Set `BASED_TEST_PG_URL`; skipped otherwise.

use std::env;

use based_core::{FkTarget, ObjectRef};
use based_postgres::column_catalog;
use sqlx::PgPool;

#[tokio::test]
async fn composite_foreign_keys_pair_columns_by_position() {
    let Ok(url) = env::var("BASED_TEST_PG_URL") else {
        return;
    };
    let pool = PgPool::connect(&url).await.unwrap();
    for sql in [
        "DROP SCHEMA IF EXISTS based_fk_test CASCADE",
        "CREATE SCHEMA based_fk_test",
        "CREATE TABLE based_fk_test.parent (a int, b int, PRIMARY KEY (a, b))",
        "CREATE TABLE based_fk_test.child (
            id int PRIMARY KEY,
            pa int,
            pb int,
            FOREIGN KEY (pb, pa) REFERENCES based_fk_test.parent (b, a)
        )",
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    let m = column_catalog(&pool, "based_fk_test", "child")
        .await
        .unwrap();
    let target = |column: &str| FkTarget {
        table: ObjectRef::qualified("based_fk_test", "parent"),
        column: column.into(),
    };
    assert_eq!(m["pa"].fk, Some(target("a")));
    assert_eq!(m["pb"].fk, Some(target("b")));
    assert_eq!(m["id"].fk, None);
    sqlx::raw_sql("DROP SCHEMA based_fk_test CASCADE")
        .execute(&pool)
        .await
        .unwrap();
}
