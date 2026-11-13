//! A table's structure from the Postgres catalog: column types as `format_type`
//! prints them, defaults via `pg_get_expr`, keys and references from
//! `pg_constraint`, index columns via `pg_get_indexdef`.

use anyhow::{Result, anyhow};
use based_core::{
    ColumnRef, ColumnSpec, ConstraintSpec, IndexSpec, ObjectRef, Reference, TableSizes, TableSpec,
};
use sqlx::{PgPool, Row};

/// The columns' names, in key order, for a constraint's `conkey` or `confkey`.
const KEY_NAMES: &str = "ARRAY(SELECT a.attname FROM unnest(con.{key}) WITH ORDINALITY k(n, o) \
     JOIN pg_attribute a ON a.attrelid = con.{rel} AND a.attnum = k.n ORDER BY k.o)::text[]";

fn key_names(key: &str, rel: &str) -> String {
    KEY_NAMES.replace("{key}", key).replace("{rel}", rel)
}

pub async fn table_spec(pool: &PgPool, schema: &str, table: &str) -> Result<TableSpec> {
    let rel = sqlx::query(
        r"SELECT c.oid::int8 AS oid, c.relkind::text AS relkind,
                 quote_ident(n.nspname) || '.' || quote_ident(c.relname) AS qualified,
                 c.reltuples::int8 AS reltuples,
                 pg_size_pretty(pg_relation_size(c.oid)) AS table_size,
                 pg_size_pretty(pg_indexes_size(c.oid)) AS indexes_size,
                 pg_size_pretty(pg_total_relation_size(c.oid)) AS total_size,
                 CASE WHEN c.relkind IN ('v', 'm') THEN pg_get_viewdef(c.oid, true) END AS viewdef
          FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE n.nspname = $1 AND c.relname = $2",
    )
    .bind(schema)
    .bind(table)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("{schema}.{table} not found"))?;
    let oid: i64 = rel.try_get("oid")?;
    let relkind: String = rel.try_get("relkind")?;
    let qualified: String = rel.try_get("qualified")?;

    let column_rows = sqlx::query(
        r"SELECT quote_ident(a.attname) AS qname, a.attname::text AS name,
                 format_type(a.atttypid, a.atttypmod) AS data_type,
                 NOT a.attnotnull AS nullable,
                 pg_get_expr(d.adbin, d.adrelid) AS default_expr,
                 a.attidentity::text AS identity, a.attgenerated::text AS generated
          FROM pg_attribute a
          LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
          WHERE a.attrelid = $1::oid AND a.attnum > 0 AND NOT a.attisdropped
          ORDER BY a.attnum",
    )
    .bind(oid)
    .fetch_all(pool)
    .await?;

    // NOT NULL constraints (Postgres 18) are shown on their columns instead.
    let constraint_rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        r"SELECT quote_ident(con.conname) AS qname, con.conname::text AS name,
                 con.contype::text AS contype, pg_get_constraintdef(con.oid) AS def,
                 {cols} AS cols, fn.nspname::text AS fschema, fc.relname::text AS ftable,
                 {fcols} AS fcols
          FROM pg_constraint con
          LEFT JOIN pg_class fc ON fc.oid = con.confrelid
          LEFT JOIN pg_namespace fn ON fn.oid = fc.relnamespace
          WHERE con.conrelid = $1::oid AND con.contype <> 'n'
          ORDER BY con.contype, con.conname",
        cols = key_names("conkey", "conrelid"),
        fcols = key_names("confkey", "confrelid"),
    )))
    .bind(oid)
    .fetch_all(pool)
    .await?;

    let index_rows = sqlx::query(
        r"SELECT i.relname::text AS name, ix.indisunique AS uniq, am.amname::text AS method,
                 ARRAY(SELECT pg_get_indexdef(ix.indexrelid, k, true)
                       FROM generate_series(1, ix.indnkeyatts) k ORDER BY k)::text[] AS cols,
                 pg_get_indexdef(ix.indexrelid) AS def,
                 EXISTS (SELECT 1 FROM pg_constraint pc
                         WHERE pc.conindid = ix.indexrelid AND pc.conrelid = ix.indrelid) AS backs
          FROM pg_index ix
          JOIN pg_class i ON i.oid = ix.indexrelid
          JOIN pg_am am ON am.oid = i.relam
          WHERE ix.indrelid = $1::oid
          ORDER BY i.relname",
    )
    .bind(oid)
    .fetch_all(pool)
    .await?;

    let reference_rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        r"SELECT n.nspname::text AS schema, c.relname::text AS tbl,
                 {cols} AS from_cols, {fcols} AS to_cols
          FROM pg_constraint con
          JOIN pg_class c ON c.oid = con.conrelid
          JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE con.contype = 'f' AND con.confrelid = $1::oid
          ORDER BY n.nspname, c.relname",
        cols = key_names("conkey", "conrelid"),
        fcols = key_names("confkey", "confrelid"),
    )))
    .bind(oid)
    .fetch_all(pool)
    .await?;

    let mut constraints = Vec::new();
    let mut primary: Vec<String> = Vec::new();
    let mut foreign: Vec<(String, ColumnRef)> = Vec::new();
    let mut ddl_constraints = Vec::new();
    for r in &constraint_rows {
        let contype: String = r.try_get("contype")?;
        let def: String = r.try_get("def")?;
        let cols: Vec<String> = r.try_get("cols")?;
        match contype.as_str() {
            "p" => primary.extend(cols.iter().cloned()),
            "f" if cols.len() == 1 => {
                let fcols: Vec<String> = r.try_get("fcols")?;
                let schema: Option<String> = r.try_get("fschema")?;
                let ftable: Option<String> = r.try_get("ftable")?;
                if let (Some(schema), Some(ftable), Some(fcol)) = (schema, ftable, fcols.first()) {
                    foreign.push((
                        cols[0].clone(),
                        ColumnRef {
                            table: ObjectRef::qualified(schema, ftable),
                            column: fcol.clone(),
                        },
                    ));
                }
            }
            _ => {}
        }
        let qname: String = r.try_get("qname")?;
        ddl_constraints.push(format!("CONSTRAINT {qname} {def}"));
        constraints.push(ConstraintSpec {
            name: r.try_get("name")?,
            kind: constraint_kind(&contype).to_owned(),
            definition: def,
        });
    }

    let mut columns = Vec::with_capacity(column_rows.len());
    let mut ddl_columns = Vec::with_capacity(column_rows.len());
    for r in &column_rows {
        let name: String = r.try_get("name")?;
        let data_type: String = r.try_get("data_type")?;
        let nullable: bool = r.try_get("nullable")?;
        let default = column_default(
            r.try_get("default_expr")?,
            &r.try_get::<String, _>("identity")?,
            &r.try_get::<String, _>("generated")?,
        );
        let qname: String = r.try_get("qname")?;
        let mut line = format!("{qname} {data_type}");
        if let Some(d) = &default {
            line.push_str(if d.starts_with("GENERATED") {
                " "
            } else {
                " DEFAULT "
            });
            line.push_str(d);
        }
        if !nullable {
            line.push_str(" NOT NULL");
        }
        ddl_columns.push(line);
        columns.push(ColumnSpec {
            primary_key: primary.contains(&name),
            references: foreign
                .iter()
                .find(|(c, _)| *c == name)
                .map(|(_, target)| target.clone()),
            name,
            data_type,
            nullable,
            default,
        });
    }

    let mut indexes = Vec::with_capacity(index_rows.len());
    let mut ddl_indexes = Vec::new();
    for r in &index_rows {
        if !r.try_get::<bool, _>("backs")? {
            ddl_indexes.push(format!("{};", r.try_get::<String, _>("def")?));
        }
        indexes.push(IndexSpec {
            name: r.try_get("name")?,
            unique: r.try_get("uniq")?,
            method: Some(r.try_get("method")?),
            columns: r.try_get("cols")?,
        });
    }

    let referenced_by = reference_rows
        .iter()
        .map(|r| {
            Ok(Reference {
                from_table: ObjectRef::qualified(
                    r.try_get::<String, _>("schema")?,
                    r.try_get::<String, _>("tbl")?,
                ),
                from_columns: r.try_get("from_cols")?,
                to_columns: r.try_get("to_cols")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let ddl = match rel.try_get::<Option<String>, _>("viewdef")? {
        Some(view) => {
            let kind = if relkind == "m" {
                "MATERIALIZED VIEW"
            } else {
                "VIEW"
            };
            format!("CREATE {kind} {qualified} AS\n{}", view.trim_end())
        }
        None => {
            let body: Vec<String> = ddl_columns.into_iter().chain(ddl_constraints).collect();
            let mut ddl = format!("CREATE TABLE {qualified} (\n  {}\n);", body.join(",\n  "));
            for index in ddl_indexes {
                ddl.push('\n');
                ddl.push_str(&index);
            }
            ddl
        }
    };

    // -1 means never analyzed (Postgres 14+); 0 may be real or never analyzed.
    let row_estimate = rel.try_get::<i64, _>("reltuples").ok().filter(|n| *n >= 0);
    let sizes = Some(TableSizes {
        table: rel.try_get("table_size")?,
        indexes: rel.try_get("indexes_size")?,
        total: rel.try_get("total_size")?,
    });

    Ok(TableSpec {
        columns,
        indexes,
        constraints,
        referenced_by,
        row_estimate,
        sizes,
        ddl,
    })
}

fn constraint_kind(contype: &str) -> &'static str {
    match contype {
        "p" => "PRIMARY KEY",
        "f" => "FOREIGN KEY",
        "u" => "UNIQUE",
        "c" => "CHECK",
        "x" => "EXCLUDE",
        "t" => "TRIGGER",
        _ => "OTHER",
    }
}

/// The default as written in DDL, including identity and generated columns.
fn column_default(expr: Option<String>, identity: &str, generated: &str) -> Option<String> {
    match (identity, generated) {
        ("a", _) => Some("GENERATED ALWAYS AS IDENTITY".into()),
        ("d", _) => Some("GENERATED BY DEFAULT AS IDENTITY".into()),
        (_, "s") => expr.map(|e| format!("GENERATED ALWAYS AS ({e}) STORED")),
        (_, "v") => expr.map(|e| format!("GENERATED ALWAYS AS ({e}) VIRTUAL")),
        _ => expr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_generated_columns_read_as_ddl() {
        assert_eq!(
            column_default(None, "a", "").as_deref(),
            Some("GENERATED ALWAYS AS IDENTITY")
        );
        assert_eq!(
            column_default(Some("a + 1".into()), "", "s").as_deref(),
            Some("GENERATED ALWAYS AS (a + 1) STORED")
        );
        assert_eq!(
            column_default(Some("now()".into()), "", "").as_deref(),
            Some("now()")
        );
    }
}
