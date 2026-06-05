use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gpui::{App, Context, Entity, SharedString, Task, Window, prelude::*, px};
use gpui_component::{
    ActiveTheme, IndexPath, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    list::{ListDelegate, ListItem, ListState},
};

use crate::connection::{ConnectionId, EngineKind};
use crate::widgets::SIDEBAR_INSET;
use crate::widgets::empty_state::pane_empty_hint;
use crate::widgets::list_row::{SchemaRowStyle, schema_object_row_with_actions};
use crate::widgets::section_eyebrow::section_eyebrow_counted;

use super::ConnectionTree;
use super::types::{ActiveObjects, ObjectKind, SchemaObject};

#[derive(Clone)]
pub(crate) struct ObjectSection {
    pub(crate) name: SharedString,
    pub(crate) items: Vec<SchemaObject>,
}

pub(crate) struct ObjectListDelegate {
    tree: Entity<ConnectionTree>,
    sections: Vec<ObjectSection>,
    engine: Option<EngineKind>,
    conn_id_for_tabs: Option<ConnectionId>,
    selected_index: Option<IndexPath>,
    loading: bool,
    empty_message: SharedString,
    all_sections: Vec<ObjectSection>,
}

impl ObjectListDelegate {
    pub(crate) fn new(tree: Entity<ConnectionTree>) -> Self {
        Self {
            tree,
            sections: Vec::new(),
            engine: None,
            conn_id_for_tabs: None,
            selected_index: None,
            loading: false,
            empty_message: "Select a connected database to browse objects.".into(),
            all_sections: Vec::new(),
        }
    }

    pub(crate) fn sync(
        &mut self,
        active: ActiveObjects,
        selected_object: Option<String>,
        conn_id_for_tabs: Option<ConnectionId>,
    ) {
        self.conn_id_for_tabs = conn_id_for_tabs;
        self.selected_index = None;
        self.loading = false;
        self.engine = None;
        self.sections.clear();
        self.all_sections.clear();

        match active {
            ActiveObjects::Empty => {
                self.empty_message = "Select a connected database to browse objects.".into();
            }
            ActiveObjects::Loading { .. } => {
                self.loading = true;
                self.empty_message = "Loading objects...".into();
            }
            ActiveObjects::Error { label, message } => {
                self.empty_message = format!(
                    "Could not load {label}: {}",
                    super::notify::error_one_liner(&message)
                )
                .into();
            }
            ActiveObjects::Ready {
                engine, objects, ..
            } => {
                self.engine = Some(engine);
                self.all_sections = group_by_kind(objects);
                self.sections = self.all_sections.clone();
                if let Some(name) = selected_object {
                    self.selected_index = index_for_object(&self.sections, &name);
                }
            }
        }
    }

    fn object_at(&self, ix: IndexPath) -> Option<&SchemaObject> {
        self.sections.get(ix.section)?.items.get(ix.row)
    }

    fn row_style(&self, cx: &App) -> SchemaRowStyle {
        SchemaRowStyle {
            muted: cx.theme().muted_foreground,
            fg: cx.theme().sidebar_foreground,
            mono_family: crate::app::prefs::code_font_family(cx),
            row_py: crate::widgets::sidebar_row_padding_y(cx),
            row_gap: crate::widgets::sidebar_row_inner_gap(cx),
        }
    }
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

fn index_for_object(sections: &[ObjectSection], display_name: &str) -> Option<IndexPath> {
    for (section, sec) in sections.iter().enumerate() {
        if let Some(row) = sec
            .items
            .iter()
            .position(|o| o.display_name() == display_name)
        {
            return Some(IndexPath::new(row).section(section));
        }
    }
    None
}

fn object_row_key(object: &SchemaObject) -> u64 {
    let object_id = object.display_name();
    let mut hasher = DefaultHasher::new();
    object_id.hash(&mut hasher);
    object.kind.label().hash(&mut hasher);
    hasher.finish()
}

impl ListDelegate for ObjectListDelegate {
    type Item = ListItem;

    fn sections_count(&self, _: &App) -> usize {
        self.sections.len()
    }

    fn items_count(&self, section: usize, _: &App) -> usize {
        self.sections
            .get(section)
            .map(|s| s.items.len())
            .unwrap_or(0)
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            self.sections = self.all_sections.clone();
        } else {
            self.sections = self
                .all_sections
                .iter()
                .map(|sec| ObjectSection {
                    name: sec.name.clone(),
                    items: sec
                        .items
                        .iter()
                        .filter(|o| o.display_name().to_lowercase().contains(&q))
                        .cloned()
                        .collect(),
                })
                .filter(|sec| !sec.items.is_empty())
                .collect();
        }
        Task::ready(())
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let sec = self.sections.get(section)?;
        Some(section_eyebrow_counted(
            sec.name.clone(),
            sec.items.len(),
            cx,
        ))
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let object = self.object_at(ix)?.clone();
        let engine = self.engine?;
        let object_id = object.display_name();
        let object_key = object_row_key(&object);
        let is_selected = self.selected_index == Some(ix);
        let object_id_label: SharedString = object_id.clone().into();
        let style = self.row_style(cx);

        let show_inspect = self.conn_id_for_tabs.is_some()
            && matches!(
                object.kind,
                ObjectKind::Table
                    | ObjectKind::View
                    | ObjectKind::MaterializedView
                    | ObjectKind::Collection
            );
        let show_insert = self.conn_id_for_tabs.is_some()
            && matches!(engine, EngineKind::MongoDB)
            && matches!(object.kind, ObjectKind::Collection);

        let mut actions = h_flex().gap_1();
        if show_inspect {
            let cid = self.conn_id_for_tabs.clone().unwrap();
            let o = object.clone();
            let tree = self.tree.clone();
            actions = actions.child(
                Button::new(("obj-inspect", object_key))
                    .small()
                    .ghost()
                    .label("◇")
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        tree.update(cx, |tree, cx| {
                            tree.open_inspector_tab(o.clone(), cid.clone(), cx);
                        });
                    }),
            );
        }
        if show_insert {
            let cid = self.conn_id_for_tabs.clone().unwrap();
            let o = object.clone();
            let tree = self.tree.clone();
            actions = actions.child(
                Button::new(("obj-insert", object_key))
                    .small()
                    .ghost()
                    .label("+")
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        tree.update(cx, |tree, cx| {
                            tree.open_document_insert_tab(o.clone(), cid.clone(), cx);
                        });
                    }),
            );
        }

        Some(
            schema_object_row_with_actions(
                ("object-row", object_key),
                is_selected,
                object.kind.list_icon(),
                object_id_label,
                style,
                actions,
            )
            .px(px(SIDEBAR_INSET))
            .mb(gpui::px(1.0)),
        )
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        pane_empty_hint(self.empty_message.clone(), cx)
    }

    fn loading(&self, _: &App) -> bool {
        self.loading
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        let selected_name = ix.and_then(|i| self.object_at(i).map(SchemaObject::display_name));
        self.tree.update(cx, |tree, cx| {
            tree.selected_object = selected_name;
            cx.notify();
        });
        cx.notify();
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(ix) = self.selected_index else {
            return;
        };
        let Some(object) = self.object_at(ix).cloned() else {
            return;
        };
        self.tree.update(cx, |tree, cx| {
            tree.on_object_clicked(object, window, cx);
        });
    }
}

pub(crate) fn ensure_object_list(
    tree: &mut ConnectionTree,
    window: &mut Window,
    cx: &mut Context<ConnectionTree>,
) -> Entity<ListState<ObjectListDelegate>> {
    if let Some(list) = tree.object_list.clone() {
        return list;
    }

    let tree_entity = cx.entity();
    let delegate = ObjectListDelegate::new(tree_entity.clone());
    let list = cx.new(|cx| {
        ListState::new(delegate, window, cx)
            .searchable(true)
            .selectable(true)
    });

    tree.object_list = Some(list.clone());
    list
}

pub(crate) fn refresh_object_list(tree: &mut ConnectionTree, cx: &mut Context<ConnectionTree>) {
    if tree.object_list_last_synced == tree.object_list_epoch {
        return;
    }
    let Some(list) = tree.object_list.clone() else {
        return;
    };
    let conn_id = tree.conn_id_for_object_tabs(cx);
    let active = tree.active_objects.clone();
    let selected = tree.selected_object.clone();
    list.update(cx, |list, cx| {
        list.delegate_mut().sync(active, selected, conn_id);
        cx.notify();
    });
    tree.object_list_last_synced = tree.object_list_epoch;
}

impl ConnectionTree {
    pub(crate) fn conn_id_for_object_tabs(&self, cx: &gpui::App) -> Option<ConnectionId> {
        self.selected_connection.and_then(|idx| {
            self.registry
                .read(cx)
                .connections()
                .get(idx)
                .map(|e| e.read(cx).id.clone())
        })
    }
}

#[cfg(test)]
mod tests {
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
