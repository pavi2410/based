use std::fmt::Write;

use based_core::EngineKind;

use super::snapshot::{ContextLevel, Entity, EntityKind, Field, SchemaSnapshot};

#[derive(Debug, Clone)]
pub struct RenderOptions<'a> {
    pub engine: EngineKind,
    pub level: ContextLevel,
    /// Soft cap on output size. Entities past it are listed by name only, then dropped.
    pub max_chars: usize,
    /// Entities to render first and in full (e.g. the table the user selected).
    pub focus: &'a [String],
}

impl<'a> RenderOptions<'a> {
    pub fn new(engine: EngineKind, level: ContextLevel) -> Self {
        Self {
            engine,
            level,
            max_chars: 12_000,
            focus: &[],
        }
    }
}

/// Renders a compact, prompt-ready description. Empty when the level is `Off`.
pub fn render(snapshot: &SchemaSnapshot, opts: &RenderOptions) -> String {
    if opts.level == ContextLevel::Off || snapshot.entities.is_empty() {
        return String::new();
    }
    let mut ordered: Vec<&Entity> = snapshot.entities.iter().collect();
    ordered.sort_by_key(|e| !opts.focus.contains(&e.name));

    let mut out = String::new();
    let mut compact = false;
    let mut dropped = 0;
    for e in ordered {
        let line = if compact {
            names_only(e)
        } else {
            full(e, opts)
        };
        if out.len() + line.len() > opts.max_chars {
            if compact {
                dropped += 1;
                continue;
            }
            compact = true;
            let line = names_only(e);
            if out.len() + line.len() > opts.max_chars {
                dropped += 1;
                continue;
            }
            out.push_str(&line);
            continue;
        }
        out.push_str(&line);
    }
    if dropped > 0 {
        let _ = writeln!(out, "... {dropped} more omitted");
    }
    out
}

fn names_only(e: &Entity) -> String {
    format!("{}\n", e.name)
}

fn full(e: &Entity, opts: &RenderOptions) -> String {
    let fields: Vec<String> = e.fields.iter().map(field).collect();
    let tag = match e.kind {
        EntityKind::View => "view ",
        EntityKind::Collection => "collection ",
        EntityKind::KeyPattern => "keys ",
        EntityKind::Table => "",
    };
    let mut s = format!("{tag}{}({})\n", e.name, fields.join(", "));
    if opts.level == ContextLevel::SchemaRows {
        for row in &e.samples {
            let _ = writeln!(s, "  e.g. {}", row.join(" | "));
        }
    }
    s
}

fn field(f: &Field) -> String {
    let mut s = f.name.clone();
    if let Some(t) = &f.ty {
        let _ = write!(s, " {t}");
    }
    if f.primary_key {
        s.push_str(" pk");
    }
    if f.nullable {
        s.push_str(" null");
    }
    if let Some(r) = &f.references {
        let _ = write!(s, " ->{r}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str) -> Field {
        Field {
            name: name.into(),
            ty: Some(ty.into()),
            ..Field::default()
        }
    }

    fn snap() -> SchemaSnapshot {
        let mut orders = Entity::new(
            "orders",
            EntityKind::Table,
            vec![
                Field {
                    primary_key: true,
                    ..col("id", "int")
                },
                Field {
                    references: Some("users.id".into()),
                    ..col("user_id", "int")
                },
                Field {
                    nullable: true,
                    ..col("note", "text")
                },
            ],
        );
        orders.samples = vec![vec!["1".into(), "7".into(), "hi".into()]];
        SchemaSnapshot {
            entities: vec![
                Entity::new("users", EntityKind::Table, vec![col("id", "int")]),
                orders,
            ],
        }
    }

    #[test]
    fn renders_compact_lines() {
        let opts = RenderOptions::new(EngineKind::Postgres, ContextLevel::Schema);
        let out = render(&snap(), &opts);
        assert_eq!(
            out,
            "users(id int)\norders(id int pk, user_id int ->users.id, note text null)\n"
        );
    }

    #[test]
    fn off_renders_nothing_and_rows_are_opt_in() {
        let mut opts = RenderOptions::new(EngineKind::Postgres, ContextLevel::Off);
        assert!(render(&snap(), &opts).is_empty());
        opts.level = ContextLevel::Schema;
        assert!(!render(&snap(), &opts).contains("e.g."));
        opts.level = ContextLevel::SchemaRows;
        assert!(render(&snap(), &opts).contains("e.g. 1 | 7 | hi"));
    }

    #[test]
    fn focus_goes_first() {
        let focus = ["orders".to_owned()];
        let mut opts = RenderOptions::new(EngineKind::Postgres, ContextLevel::Schema);
        opts.focus = &focus;
        assert!(render(&snap(), &opts).starts_with("orders("));
    }

    #[test]
    fn budget_degrades_to_names_then_drops() {
        let mut opts = RenderOptions::new(EngineKind::Postgres, ContextLevel::Schema);
        opts.max_chars = 21;
        let out = render(&snap(), &opts);
        assert!(out.contains("users(id int)"));
        assert!(out.contains("orders\n"));
        assert!(!out.contains("user_id"));

        opts.max_chars = 14;
        let out = render(&snap(), &opts);
        assert!(out.contains("more omitted"));
    }
}
