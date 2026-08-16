use gpui::SharedString;

use super::types::SchemaObject;

#[derive(Clone)]
pub(crate) struct ObjectSection {
    pub(crate) name: SharedString,
    pub(crate) items: Vec<SchemaObject>,
}

#[derive(Clone)]
pub(crate) struct SchemaSection {
    pub(crate) name: SharedString,
    pub(crate) kinds: Vec<ObjectSection>,
}

pub(crate) fn group_by_kind(objects: Vec<SchemaObject>) -> Vec<ObjectSection> {
    let mut groups: Vec<(&'static str, Vec<SchemaObject>)> = Vec::new();
    for object in objects {
        let group = object.kind.group();
        if let Some((_, rows)) = groups.iter_mut().find(|(name, _)| *name == group) {
            rows.push(object);
        } else {
            groups.push((group, vec![object]));
        }
    }
    groups
        .into_iter()
        .map(|(name, items)| ObjectSection {
            name: name.into(),
            items,
        })
        .collect()
}

pub(crate) fn group_postgres_objects(objects: Vec<SchemaObject>) -> Vec<SchemaSection> {
    use std::collections::BTreeMap;

    let mut by_schema: BTreeMap<String, Vec<SchemaObject>> = BTreeMap::new();
    for object in objects {
        let schema = object.schema.clone().unwrap_or_else(|| "public".into());
        by_schema.entry(schema).or_default().push(object);
    }

    by_schema
        .into_iter()
        .map(|(name, objects)| {
            let mut kinds = group_by_kind(objects);
            kinds.sort_by_key(|a| kind_section_order(&a.name));
            for section in &mut kinds {
                section.items.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SchemaSection {
                name: name.into(),
                kinds,
            }
        })
        .collect()
}

fn kind_section_order(name: &str) -> u8 {
    match name {
        "Tables" => 0,
        "Views" => 1,
        "Triggers" => 2,
        "Collections" => 3,
        _ => 4,
    }
}

pub(crate) fn object_matches_query(object: &SchemaObject, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    let q = q.to_lowercase();
    object.display_name().to_lowercase().contains(&q)
        || object.name.to_lowercase().contains(&q)
        || object
            .schema
            .as_ref()
            .is_some_and(|s| s.to_lowercase().contains(&q))
}

#[cfg(test)]
mod tests {
    use super::super::types::ObjectKind;
    use super::*;

    fn pg_table(schema: &str, name: &str) -> SchemaObject {
        SchemaObject {
            name: name.into(),
            schema: Some(schema.into()),
            kind: ObjectKind::Table,
        }
    }

    fn pg_view(schema: &str, name: &str) -> SchemaObject {
        SchemaObject {
            name: name.into(),
            schema: Some(schema.into()),
            kind: ObjectKind::View,
        }
    }

    fn pg_matview(schema: &str, name: &str) -> SchemaObject {
        SchemaObject {
            name: name.into(),
            schema: Some(schema.into()),
            kind: ObjectKind::MaterializedView,
        }
    }

    #[test]
    fn group_postgres_objects_sorts_schemas_and_kinds() {
        let objects = vec![
            pg_table("auth", "sessions"),
            pg_view("public", "active_users"),
            pg_table("public", "orders"),
            pg_table("public", "users"),
            pg_matview("public", "summary"),
        ];
        let sections = group_postgres_objects(objects);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name.as_ref(), "auth");
        assert_eq!(sections[1].name.as_ref(), "public");
        let public_kinds = &sections[1].kinds;
        assert_eq!(public_kinds.len(), 2);
        assert_eq!(public_kinds[0].name.as_ref(), "Tables");
        assert_eq!(public_kinds[1].name.as_ref(), "Views");
        assert_eq!(
            public_kinds[0]
                .items
                .iter()
                .map(|o| o.name.as_str())
                .collect::<Vec<_>>(),
            vec!["orders", "users"]
        );
        assert_eq!(
            public_kinds[1]
                .items
                .iter()
                .map(|o| o.name.as_str())
                .collect::<Vec<_>>(),
            vec!["active_users", "summary"]
        );
    }

    #[test]
    fn group_postgres_objects_empty_input() {
        assert!(group_postgres_objects(vec![]).is_empty());
    }

    #[test]
    fn flatten_schema_objects_sorts_names_without_kind_headers() {
        let objects = vec![
            pg_view("public", "active_users"),
            pg_table("public", "orders"),
            pg_table("public", "users"),
        ];
        let sections = group_postgres_objects(objects);
        let mut flat: Vec<&str> = sections[0]
            .kinds
            .iter()
            .flat_map(|kind| kind.items.iter().map(|o| o.name.as_str()))
            .collect();
        flat.sort_unstable();
        assert_eq!(flat, vec!["active_users", "orders", "users"]);
        assert!(
            !flat
                .iter()
                .any(|name| *name == "Tables" || *name == "Views"),
            "kind group titles must not appear as object names"
        );
    }

    #[test]
    fn group_by_kind_schema_less_objects() {
        let objects = vec![
            SchemaObject {
                name: "notes".into(),
                schema: None,
                kind: ObjectKind::Table,
            },
            SchemaObject {
                name: "events".into(),
                schema: None,
                kind: ObjectKind::Collection,
            },
        ];
        let sections = group_by_kind(objects);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name.as_ref(), "Tables");
        assert_eq!(sections[1].name.as_ref(), "Collections");
    }
}
