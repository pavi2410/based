//! Editor schema cache -> prompt context, honouring the connection's `[ai]` level.

use based_ai::context::{
    ContextLevel, Entity, EntityKind, Field, RenderOptions, SchemaSnapshot, render,
};
use based_core::{EngineKind, ObjectKind, Relation};

use crate::editor::SchemaCache;
use crate::editor::schema_cache::RelationText as _;

/// One table or view as a prompt entity.
pub fn entity(o: &Relation) -> Entity {
    let kind = match o.object.kind {
        ObjectKind::View | ObjectKind::MaterializedView => EntityKind::View,
        ObjectKind::Collection => EntityKind::Collection,
        ObjectKind::Key => EntityKind::KeyPattern,
        ObjectKind::Table | ObjectKind::Trigger => EntityKind::Table,
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
        .filter(|o| lower.contains(&o.label().to_lowercase()))
        .map(|o| o.display())
        .collect();
    let mut opts = RenderOptions::new(engine, level);
    opts.focus = &focus;
    render(&snapshot(cache), &opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use based_core::{DbColumn, DbObject};

    fn cache() -> SchemaCache {
        let table = |name: &str| Relation {
            object: DbObject::new(Some("public".into()), name, ObjectKind::Table),
            columns: vec![DbColumn {
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
