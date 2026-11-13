//! sqlx errors → typed connect and query errors, by SQLSTATE and error kind.

use based_core::{ConnectError, ConnectionErrorCategory as C, QueryError};
use sqlx::postgres::{PgDatabaseError, PgErrorPosition};

/// Classify a failed connect. Database errors go by SQLSTATE
/// (<https://www.postgresql.org/docs/current/errcodes-appendix.html>).
pub fn connect_error(err: sqlx::Error) -> ConnectError {
    let category = match &err {
        sqlx::Error::Database(db) => match db.code().as_deref() {
            // invalid_password, invalid_authorization_specification (pg_hba rejections)
            Some("28P01" | "28000") => C::AuthFailed,
            // invalid_catalog_name
            Some("3D000") => C::DbNotFound,
            _ => C::ServerError,
        },
        sqlx::Error::Io(io) => return ConnectError::io(io),
        sqlx::Error::Tls(_) => C::TlsRejected,
        sqlx::Error::PoolTimedOut => C::NetTimeout,
        sqlx::Error::Configuration(_) => C::ConfigInvalid,
        _ => C::ServerError,
    };
    ConnectError::new(category, err)
}

/// `err` from running `sql`, with where the server says it went wrong. Postgres
/// reports a 1-based character index into the statement it was sent.
pub fn query_error(err: sqlx::Error, sql: &str) -> QueryError {
    let position = match &err {
        sqlx::Error::Database(db) => db
            .try_downcast_ref::<PgDatabaseError>()
            .and_then(PgDatabaseError::position)
            .and_then(|p| match p {
                PgErrorPosition::Original(chars) => char_to_byte(sql, chars),
                PgErrorPosition::Internal { .. } => None,
            }),
        _ => None,
    };
    QueryError {
        message: err.to_string(),
        position,
    }
}

/// 1-based character index → byte offset.
fn char_to_byte(sql: &str, chars: usize) -> Option<usize> {
    sql.char_indices()
        .nth(chars.checked_sub(1)?)
        .map(|(b, _)| b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_positions_become_byte_offsets() {
        assert_eq!(char_to_byte("SELEC 1", 1), Some(0));
        assert_eq!(char_to_byte("select 'é', x", 13), Some(13));
        assert_eq!(char_to_byte("x", 0), None);
        assert_eq!(char_to_byte("x", 5), None);
    }
}
