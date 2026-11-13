//! Runs against a live Postgres. Set `BASED_TEST_PG_URL`; skipped otherwise.

use std::env;

use based_core::{DbObject, ObjectKind};
use based_postgres::{execute_sql, list_objects, list_relations};
use sqlx::PgPool;

#[tokio::test]
async fn lists_tables_views_and_materialized_views_with_columns() {
    let Ok(url) = env::var("BASED_TEST_PG_URL") else {
        return;
    };
    let pool = PgPool::connect(&url).await.unwrap();
    for sql in [
        "DROP SCHEMA IF EXISTS based_objects_test CASCADE",
        "CREATE SCHEMA based_objects_test",
        "CREATE TABLE based_objects_test.t (id int, name text)",
        "CREATE VIEW based_objects_test.v AS SELECT id FROM based_objects_test.t",
        "CREATE MATERIALIZED VIEW based_objects_test.m AS SELECT name FROM based_objects_test.t",
    ] {
        execute_sql(&pool, sql).await.unwrap();
    }
    let in_schema = |o: &DbObject| o.schema.as_deref() == Some("based_objects_test");

    let objects: Vec<_> = list_objects(&pool)
        .await
        .unwrap()
        .into_iter()
        .filter(in_schema)
        .map(|o| (o.name, o.kind))
        .collect();
    assert_eq!(
        objects,
        [
            ("m".to_string(), ObjectKind::MaterializedView),
            ("t".to_string(), ObjectKind::Table),
            ("v".to_string(), ObjectKind::View),
        ]
    );

    let relations: Vec<_> = list_relations(&pool)
        .await
        .unwrap()
        .into_iter()
        .filter(|r| in_schema(&r.object))
        .map(|r| {
            let cols: Vec<_> = r
                .columns
                .into_iter()
                .map(|c| (c.name, c.data_type))
                .collect();
            (r.object.name, cols)
        })
        .collect();
    let col = |n: &str, t: &str| (n.to_string(), t.to_string());
    assert_eq!(
        relations,
        [
            ("m".to_string(), vec![col("name", "text")]),
            (
                "t".to_string(),
                vec![col("id", "integer"), col("name", "text")]
            ),
            ("v".to_string(), vec![col("id", "integer")]),
        ]
    );

    execute_sql(&pool, "DROP SCHEMA based_objects_test CASCADE")
        .await
        .unwrap();
}
