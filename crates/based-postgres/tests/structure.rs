//! Runs against a live Postgres. Set `BASED_TEST_PG_URL`; skipped otherwise.

use std::env;

use based_core::ObjectRef;
use based_postgres::{execute_sql, table_spec};
use sqlx::PgPool;

#[tokio::test]
async fn reads_types_keys_indexes_references_and_ddl() {
    let Ok(url) = env::var("BASED_TEST_PG_URL") else {
        return;
    };
    let pool = PgPool::connect(&url).await.unwrap();
    for sql in [
        "DROP SCHEMA IF EXISTS based_structure_test CASCADE",
        "CREATE SCHEMA based_structure_test",
        "CREATE TABLE based_structure_test.customers (id serial PRIMARY KEY, email text UNIQUE)",
        "CREATE TABLE based_structure_test.orders (
            id int GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            customer_id int REFERENCES based_structure_test.customers (id),
            total numeric(10,2) NOT NULL DEFAULT 0,
            tags text[],
            placed_at timestamptz
         )",
        "CREATE INDEX orders_by_date ON based_structure_test.orders (placed_at, customer_id)",
        "CREATE TABLE based_structure_test.items (
            order_id int REFERENCES based_structure_test.orders (id), sku text
         )",
    ] {
        execute_sql(&pool, sql).await.unwrap();
    }

    let spec = table_spec(&pool, "based_structure_test", "orders")
        .await
        .unwrap();

    let types: Vec<(&str, &str)> = spec
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c.data_type.as_str()))
        .collect();
    assert_eq!(
        types,
        [
            ("id", "integer"),
            ("customer_id", "integer"),
            ("total", "numeric(10,2)"),
            ("tags", "text[]"),
            ("placed_at", "timestamp with time zone"),
        ]
    );
    let id = &spec.columns[0];
    assert!(id.primary_key && !id.nullable);
    assert_eq!(id.default.as_deref(), Some("GENERATED ALWAYS AS IDENTITY"));
    assert_eq!(spec.columns[2].default.as_deref(), Some("0"));

    let customer = &spec.columns[1];
    let target = customer
        .references
        .as_ref()
        .expect("customer_id is a foreign key");
    assert_eq!(
        target.table,
        ObjectRef::qualified("based_structure_test", "customers")
    );
    assert_eq!(target.column, "id");
    assert!(
        spec.is_unindexed_foreign_key(customer),
        "orders_by_date leads with placed_at"
    );
    assert_eq!(spec.indexes_on("customer_id"), ["orders_by_date"]);

    assert_eq!(spec.referenced_by.len(), 1);
    let item = &spec.referenced_by[0];
    assert_eq!(
        item.from_table,
        ObjectRef::qualified("based_structure_test", "items")
    );
    assert_eq!(
        (item.from_columns.as_slice(), item.to_columns.as_slice()),
        (&["order_id".to_string()][..], &["id".to_string()][..])
    );

    assert!(
        spec.ddl
            .starts_with("CREATE TABLE based_structure_test.orders (")
    );
    assert!(spec.ddl.contains("total numeric(10,2) DEFAULT 0 NOT NULL"));
    assert!(
        spec.ddl
            .contains("REFERENCES based_structure_test.customers(id)")
    );
    assert!(
        spec.ddl.contains("CREATE INDEX orders_by_date"),
        "indexes that back no constraint follow the table"
    );
    assert!(!spec.ddl.contains("CREATE UNIQUE INDEX orders_pkey"));

    execute_sql(&pool, "DROP SCHEMA based_structure_test CASCADE")
        .await
        .unwrap();
}
