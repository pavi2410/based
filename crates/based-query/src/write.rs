//! Does a SQL script change data or schema? It decides whether to ask before
//! running on a production connection, so it errs towards "yes": any statement
//! word that writes counts, wherever it appears (`WITH … INSERT`).

use based_core::EngineKind;

use crate::sql::{command_words, split_statements};

const WRITE_KEYWORDS: &[&str] = &[
    "insert", "update", "delete", "merge", "drop", "alter", "truncate", "create", "grant",
    "revoke", "replace", "vacuum", "reindex",
];

/// `true` when the script contains a write keyword outside strings, quoted
/// names, comments and function calls.
pub fn changes_data(script: &str, engine: EngineKind) -> bool {
    command_words(script, engine)
        .iter()
        .any(|w| WRITE_KEYWORDS.contains(&w.as_str()))
}

/// What a write does, for a confirmation: the distinct write verbs in order, and a
/// warning when an UPDATE or DELETE has no WHERE clause. Empty for a read.
pub fn write_summary(script: &str, engine: EngineKind) -> String {
    let all = command_words(script, engine);
    let mut verbs: Vec<&str> = Vec::new();
    for w in &all {
        if WRITE_KEYWORDS.contains(&w.as_str()) && !verbs.contains(&w.as_str()) {
            verbs.push(w);
        }
    }
    let unbounded =
        verbs.iter().any(|v| *v == "update" || *v == "delete") && !all.iter().any(|w| w == "where");
    let mut out = verbs
        .iter()
        .map(|v| v.to_ascii_uppercase())
        .collect::<Vec<_>>()
        .join(", ");
    if unbounded {
        out.push_str(" with no WHERE clause");
    }
    out
}

/// Whether any statement in the script destroys data wholesale: DROP, TRUNCATE, or an
/// UPDATE or DELETE without a WHERE clause. Checked per statement, so a WHERE in one
/// statement does not vouch for another.
pub fn is_destructive(script: &str, engine: EngineKind) -> bool {
    split_statements(script, engine).into_iter().any(|range| {
        let words = command_words(&script[range], engine);
        let has = |w: &str| words.iter().any(|x| x == w);
        has("drop") || has("truncate") || ((has("update") || has("delete")) && !has("where"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PG: EngineKind = EngineKind::Postgres;

    #[test]
    fn summary_names_verbs_and_unbounded_writes() {
        assert_eq!(write_summary("SELECT 1", PG), "");
        assert_eq!(write_summary("DELETE FROM t WHERE id = 1", PG), "DELETE");
        assert_eq!(
            write_summary("update t set a = 1; drop table x", PG),
            "UPDATE, DROP with no WHERE clause"
        );
    }

    #[test]
    fn destructive_statements_are_flagged_one_by_one() {
        assert!(is_destructive("delete from users", PG));
        assert!(is_destructive("UPDATE t SET a = 1", PG));
        assert!(is_destructive("drop table t", PG));
        assert!(is_destructive("truncate t", PG));
        assert!(is_destructive(
            "delete from a where id = 1; delete from b",
            PG
        ));
        assert!(!is_destructive("delete from a where id = 1", PG));
        assert!(!is_destructive("insert into t values (1)", PG));
        assert!(!is_destructive("select 'drop table t'", PG));
        assert!(!is_destructive("-- delete from t\nselect 1", PG));
    }

    #[test]
    fn writes_are_detected() {
        assert!(changes_data(
            "DELETE FROM sessions WHERE expires_at < now()",
            PG
        ));
        assert!(changes_data("  update users set plan = 'pro'", PG));
        assert!(changes_data(
            "WITH x AS (SELECT 1) INSERT INTO t SELECT * FROM x",
            PG
        ));
        assert!(changes_data("select 1; drop table t", PG));
        assert!(changes_data(
            "replace into t values (1)",
            EngineKind::SQLite
        ));
    }

    #[test]
    fn reads_and_text_are_not() {
        assert!(!changes_data("SELECT id, email FROM users LIMIT 50", PG));
        assert!(!changes_data("select 'delete everything' as note", PG));
        assert!(!changes_data("-- drop table t\nselect 1", PG));
        assert!(!changes_data("select updated_at, \"delete\" from t", PG));
        assert!(!changes_data("select /* insert */ 1", PG));
        assert!(!changes_data("select $$ drop table t $$", PG));
        assert!(!changes_data("select replace(name, 'a', 'b') from t", PG));
    }
}
