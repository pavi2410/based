//! Editor diagnostics: parser errors plus schema-aware checks.

use based_core::EngineKind;
use based_query::{Lint, Severity, statements_in_script, syntax_lints};

use super::schema_cache::SchemaCache;
use super::sql_scope::{cte_names, tables_in_scope, unfiltered_write};

fn is_system(name: &str) -> bool {
    let n = name.trim_matches('"').to_lowercase();
    n.starts_with("pg_") || n.starts_with("information_schema") || n.starts_with("sqlite_")
}

/// Lints for `sql`. Semantic checks run only when the script parses.
pub fn lint(sql: &str, engine: EngineKind, cache: &SchemaCache) -> Vec<Lint> {
    let mut out = syntax_lints(sql, engine);
    if !out.is_empty() {
        return out;
    }
    for stmt in statements_in_script(sql) {
        let raw = &sql[stmt.start..stmt.end];
        let lead = raw.len() - raw.trim_start().len();
        let text = stmt.text(sql);
        let base = stmt.start + lead;

        if let Some(kind) = unfiltered_write(text) {
            out.push(Lint {
                start: base,
                end: base + kind.len(),
                severity: Severity::Warning,
                message: format!("{kind} without WHERE affects every row"),
            });
        }
        if cache.objects.is_empty() {
            continue;
        }
        let ctes = cte_names(text);
        for t in tables_in_scope(text) {
            if is_system(&t.name)
                || ctes.iter().any(|c| c.eq_ignore_ascii_case(&t.name))
                || cache.find_by_name(&t.name).is_some()
            {
                continue;
            }
            out.push(Lint {
                start: base + t.offset,
                end: base + t.offset + t.name.len(),
                severity: Severity::Warning,
                message: format!("Unknown table {}", t.name),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::schema_cache::{ObjectKind, SchemaObject};

    fn cache() -> SchemaCache {
        let mut c = SchemaCache::new(EngineKind::Postgres);
        c.objects.push(SchemaObject {
            full_name: "public.users".into(),
            label: "users".into(),
            schema: Some("public".into()),
            kind: ObjectKind::Table,
            columns: vec![],
        });
        c
    }

    #[test]
    fn flags_unknown_table_and_unfiltered_delete() {
        let sql = "select * from users; select * from nope; delete from users";
        let l = lint(sql, EngineKind::Postgres, &cache());
        assert_eq!(l.len(), 2, "{l:?}");
        assert_eq!(&sql[l[0].start..l[0].end], "nope");
        assert_eq!(&sql[l[1].start..l[1].end], "delete");
    }

    #[test]
    fn cte_is_not_unknown() {
        let sql = "with x as (select 1) select * from x";
        assert!(lint(sql, EngineKind::Postgres, &cache()).is_empty());
    }
}
