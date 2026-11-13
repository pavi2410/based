//! Runs against a live Postgres. Set `BASED_TEST_PG_URL`; skipped otherwise.

use std::env;

use based_core::ConnectionErrorCategory;
use based_postgres::{connect_error, execute_sql};
use sqlx::{Connection, PgConnection, PgPool};

fn url() -> Option<String> {
    env::var("BASED_TEST_PG_URL").ok()
}

async fn category(url: &str) -> ConnectionErrorCategory {
    // One connection, as the app's open does first: a pool retries refusals
    // until its acquire timeout and reports PoolTimedOut instead.
    let err = PgConnection::connect(url)
        .await
        .expect_err("connect should fail");
    connect_error(err).category
}

#[tokio::test]
async fn connect_failures_are_classified_by_sqlstate_and_io_kind() {
    let Some(good) = url() else { return };
    let wrong_password = good.replacen("based:based@", "based:wrong@", 1);
    assert_eq!(
        category(&wrong_password).await,
        ConnectionErrorCategory::AuthFailed
    );
    let (base, _) = good.rsplit_once('/').unwrap();
    assert_eq!(
        category(&format!("{base}/no_such_db_based")).await,
        ConnectionErrorCategory::DbNotFound
    );
    assert_eq!(
        category("postgres://based:based@127.0.0.1:1/based").await,
        ConnectionErrorCategory::NetUnreachable
    );
}

#[tokio::test]
async fn query_errors_carry_the_servers_position() {
    let Some(good) = url() else { return };
    let pool = PgPool::connect(&good).await.unwrap();
    let sql = "SELECT 'é' AS a FROM no_such_table_based";
    let err = execute_sql(&pool, sql).await.unwrap_err();
    let at = err.position.expect("position");
    assert!(sql[at..].starts_with("no_such_table_based"), "{at}: {err}");

    let err = execute_sql(&pool, "SELEC 1").await.unwrap_err();
    assert_eq!(err.position, Some(0));
}
