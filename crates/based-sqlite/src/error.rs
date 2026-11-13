//! sqlx errors → typed connect errors, by SQLite result code
//! (<https://www.sqlite.org/rescode.html>).

use based_core::{ConnectError, ConnectionErrorCategory as C};

const SQLITE_CANTOPEN: i32 = 14;
const SQLITE_NOTADB: i32 = 26;
const SQLITE_PERM: i32 = 3;
const SQLITE_READONLY: i32 = 8;

/// Classify a failed open. sqlx reports the extended code; the low byte is
/// the primary one.
pub fn connect_error(err: sqlx::Error) -> ConnectError {
    let category = match &err {
        sqlx::Error::Database(db) => {
            let primary = db
                .code()
                .and_then(|c| c.parse::<i32>().ok())
                .map(|c| c & 0xff);
            match primary {
                Some(SQLITE_CANTOPEN) => C::DbNotFound,
                Some(SQLITE_NOTADB | SQLITE_PERM | SQLITE_READONLY) => C::ConfigInvalid,
                _ => C::ServerError,
            }
        }
        sqlx::Error::Io(io) => return ConnectError::io(io),
        sqlx::Error::Configuration(_) => C::ConfigInvalid,
        _ => C::ServerError,
    };
    ConnectError::new(category, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use sqlx::SqlitePool;
    use sqlx::sqlite::SqliteConnectOptions;

    #[tokio::test]
    async fn a_missing_file_is_not_found_and_garbage_is_not_a_database() {
        let missing = SqliteConnectOptions::new().filename("/no/such/dir/based.db");
        let err = SqlitePool::connect_with(missing).await.unwrap_err();
        assert_eq!(connect_error(err).category, C::DbNotFound);

        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("junk.db");
        fs::write(&junk, vec![b'x'; 4096]).unwrap();
        let pool = SqlitePool::connect_with(SqliteConnectOptions::new().filename(&junk))
            .await
            .unwrap();
        let err = sqlx::query("SELECT * FROM sqlite_master")
            .fetch_all(&pool)
            .await
            .unwrap_err();
        assert_eq!(connect_error(err).category, C::ConfigInvalid);
    }
}
