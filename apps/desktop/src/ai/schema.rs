//! Editor schema cache -> prompt context, honouring the connection's `[ai]` level.

use based_ai::context::{
    ContextLevel, Entity, EntityKind, Field, RenderOptions, SchemaSnapshot, render,
};
use based_core::EngineKind;
use based_project::{AiContext, AiSettings};

use crate::editor::SchemaCache;
use crate::editor::schema_cache::{ObjectKind, SchemaObject};

pub fn level(ai: Option<AiSettings>) -> ContextLevel {
    match ai.map(|a| a.effective_context()) {
        None | Some(AiContext::Off) => ContextLevel::Off,
        Some(AiContext::Schema) => ContextLevel::Schema,
        Some(AiContext::SchemaRows) => ContextLevel::SchemaRows,
    }
}

/// One table or view as a prompt entity.
pub fn entity(o: &SchemaObject) -> Entity {
    let kind = match o.kind {
        ObjectKind::Table => EntityKind::Table,
        ObjectKind::View => EntityKind::View,
    };
    let fields = o
        .columns
        .iter()
        .map(|c| Field {
            name: c.name.clone(),
            ty: Some(c.data_type.clone()),
            ..Field::default()
        })
        .collect();
    Entity::new(o.display(), kind, fields)
}

fn snapshot(cache: &SchemaCache) -> SchemaSnapshot {
    SchemaSnapshot {
        entities: cache.objects.iter().map(entity).collect(),
    }
}

/// Prompt-ready schema text. Tables named in `text` are rendered first and in full.
pub fn context(engine: EngineKind, cache: &SchemaCache, level: ContextLevel, text: &str) -> String {
    if level == ContextLevel::Off {
        return String::new();
    }
    let lower = text.to_lowercase();
    let focus: Vec<String> = cache
        .objects
        .iter()
        .filter(|o| lower.contains(&o.label.to_lowercase()))
        .map(|o| o.display())
        .collect();
    let mut opts = RenderOptions::new(engine, level);
    opts.focus = &focus;
    render(&snapshot(cache), &opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::schema_cache::{ColumnInfo, SchemaObject};

    fn cache() -> SchemaCache {
        let table = |name: &str| SchemaObject {
            full_name: format!("public.{name}"),
            label: name.into(),
            schema: Some("public".into()),
            kind: ObjectKind::Table,
            columns: vec![ColumnInfo {
                name: "id".into(),
                data_type: "int4".into(),
            }],
        };
        SchemaCache {
            engine: Some(EngineKind::Postgres),
            objects: vec![table("orders"), table("users")],
            last_refreshed_at: None,
        }
    }

    #[test]
    fn off_sends_nothing() {
        let c = cache();
        assert!(context(EngineKind::Postgres, &c, ContextLevel::Off, "users").is_empty());
    }

    #[test]
    fn disabled_connection_is_off() {
        assert_eq!(level(None), ContextLevel::Off);
        let on = AiSettings {
            enabled: true,
            context: AiContext::Schema,
        };
        assert_eq!(level(Some(on)), ContextLevel::Schema);
        let off = AiSettings {
            enabled: false,
            context: AiContext::SchemaRows,
        };
        assert_eq!(level(Some(off)), ContextLevel::Off);
    }

    #[test]
    fn mentioned_tables_come_first() {
        let c = cache();
        let out = context(
            EngineKind::Postgres,
            &c,
            ContextLevel::Schema,
            "select * from users",
        );
        let (u, o) = (out.find("users").unwrap(), out.find("orders").unwrap());
        assert!(u < o, "{out}");
    }
}
