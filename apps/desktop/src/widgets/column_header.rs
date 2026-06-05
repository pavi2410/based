//! Column header chrome: constraint icons, type tooltips.

use gpui::{App, Hsla, IntoElement, SharedString, Window, div, prelude::*, px};
use gpui_component::{ActiveTheme, Icon, Sizable as _, h_flex, tooltip::Tooltip, v_flex};

use crate::app::prefs;

pub const KEY_ICON: &str = "icons/key.svg";
pub const LINK_ICON: &str = "icons/link.svg";
pub const SHIELD_CHECK_ICON: &str = "icons/shield-check.svg";

/// Per-column metadata for grid headers (parallel to [`gpui_component::table::Column::key`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridColumnMeta {
    pub data_type: Option<String>,
    pub nullable: Option<bool>,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
    pub is_unique: bool,
    pub fk_target: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnRole {
    PrimaryKey,
    ForeignKey,
    Unique,
}

/// Highest-priority constraint role for the leading header icon (PK > FK > UQ).
pub fn dominant_role(meta: &GridColumnMeta) -> Option<ColumnRole> {
    if meta.is_primary_key {
        Some(ColumnRole::PrimaryKey)
    } else if meta.is_foreign_key {
        Some(ColumnRole::ForeignKey)
    } else if meta.is_unique {
        Some(ColumnRole::Unique)
    } else {
        None
    }
}

pub fn role_icon_path(role: ColumnRole) -> &'static str {
    match role {
        ColumnRole::PrimaryKey => KEY_ICON,
        ColumnRole::ForeignKey => LINK_ICON,
        ColumnRole::Unique => SHIELD_CHECK_ICON,
    }
}

pub fn role_icon_color(role: ColumnRole, cx: &App) -> Hsla {
    let t = cx.theme();
    match role {
        ColumnRole::PrimaryKey => t.blue_light,
        ColumnRole::ForeignKey => t.magenta_light,
        ColumnRole::Unique => t.green_light,
    }
}

pub fn has_tooltip_content(meta: &GridColumnMeta) -> bool {
    meta.data_type.is_some()
        || meta.fk_target.is_some()
        || meta.is_primary_key
        || meta.is_foreign_key
        || meta.is_unique
        || meta.nullable == Some(false)
}

/// Text lines shown in the header hover tooltip.
pub fn tooltip_lines(meta: &GridColumnMeta) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(ty) = &meta.data_type
        && !ty.is_empty()
    {
        lines.push(ty.clone());
    }
    if meta.nullable == Some(false) {
        lines.push("NOT NULL".into());
    }

    let mut roles = Vec::new();
    if meta.is_primary_key {
        roles.push("Primary key".to_string());
    }
    if meta.is_foreign_key {
        if let Some(target) = &meta.fk_target {
            roles.push(format!("Foreign key → {target}"));
        } else {
            roles.push("Foreign key".to_string());
        }
    }
    if meta.is_unique {
        roles.push("Unique".to_string());
    }
    if !roles.is_empty() {
        lines.push(roles.join(" · "));
    }
    lines
}

pub fn meta_for_column_name(name: &str) -> GridColumnMeta {
    let mut meta = GridColumnMeta::default();
    if name == "_id" {
        meta.is_primary_key = true;
    }
    meta
}

pub fn align_meta_to_columns(
    column_keys: impl IntoIterator<Item = impl AsRef<str>>,
    catalog: &std::collections::HashMap<String, GridColumnMeta>,
) -> Vec<GridColumnMeta> {
    column_keys
        .into_iter()
        .map(|k| {
            let key = k.as_ref();
            catalog
                .get(key)
                .cloned()
                .unwrap_or_else(|| meta_for_column_name(key))
        })
        .collect()
}

pub fn meta_from_query_type(type_name: impl Into<String>) -> GridColumnMeta {
    GridColumnMeta {
        data_type: Some(type_name.into()),
        ..Default::default()
    }
}

/// Reorder column metadata when a column is moved (mirrors `RowDelegate::move_column`).
pub fn reorder_column_meta(meta: &mut Vec<GridColumnMeta>, col_ix: usize, to_ix: usize) {
    if col_ix >= meta.len() || to_ix > meta.len() {
        return;
    }
    let item = meta.remove(col_ix);
    let insert_at = if to_ix > col_ix { to_ix - 1 } else { to_ix };
    meta.insert(insert_at.min(meta.len()), item);
}

pub fn render_column_header(
    col_ix: usize,
    name: SharedString,
    meta: GridColumnMeta,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let role = dominant_role(&meta);
    let show_tooltip = has_tooltip_content(&meta);
    let tooltip_lines = tooltip_lines(&meta);

    let mut header = h_flex()
        .id(("grid-col-header", col_ix))
        .flex_1()
        .min_w_0()
        .h_full()
        .gap(px(4.0))
        .items_center();

    if let Some(role) = role {
        header = header.child(
            Icon::empty()
                .path(role_icon_path(role))
                .xsmall()
                .text_color(role_icon_color(role, cx)),
        );
    }

    header = header.child(
        div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .truncate()
            .font_family(prefs::code_font_family(cx))
            .child(name),
    );

    if show_tooltip {
        let has_type_line = meta.data_type.is_some();
        let lines = tooltip_lines;
        header = header.hoverable_tooltip(move |window, app| {
            Tooltip::element({
                let lines = lines.clone();
                move |_w, tip_cx| {
                    let fg = tip_cx.theme().foreground;
                    let subtle = tip_cx.theme().muted_foreground;
                    let mono = prefs::code_font_family(tip_cx);
                    let mut col = v_flex().gap_1().max_w(px(420.0));
                    for (i, line) in lines.iter().enumerate() {
                        let is_type = has_type_line && i == 0;
                        col = col.child(
                            div()
                                .text_xs()
                                .when(is_type, |el| el.font_family(mono.clone()))
                                .text_color(if is_type { fg } else { subtle })
                                .child(line.clone()),
                        );
                    }
                    col
                }
            })
            .build(window, app)
        });
    }

    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_role_priority() {
        let pk_fk = GridColumnMeta {
            is_primary_key: true,
            is_foreign_key: true,
            is_unique: true,
            ..Default::default()
        };
        assert_eq!(dominant_role(&pk_fk), Some(ColumnRole::PrimaryKey));

        let fk_uq = GridColumnMeta {
            is_foreign_key: true,
            is_unique: true,
            ..Default::default()
        };
        assert_eq!(dominant_role(&fk_uq), Some(ColumnRole::ForeignKey));

        let uq = GridColumnMeta {
            is_unique: true,
            ..Default::default()
        };
        assert_eq!(dominant_role(&uq), Some(ColumnRole::Unique));

        assert_eq!(dominant_role(&GridColumnMeta::default()), None);
    }

    #[test]
    fn tooltip_lists_secondary_constraints() {
        let meta = GridColumnMeta {
            data_type: Some("integer".into()),
            is_primary_key: true,
            is_foreign_key: true,
            fk_target: Some("public.accounts(id)".into()),
            is_unique: true,
            ..Default::default()
        };
        let lines = tooltip_lines(&meta);
        assert_eq!(lines[0], "integer");
        assert!(lines[1].contains("Primary key"));
        assert!(lines[1].contains("Foreign key → public.accounts(id)"));
        assert!(lines[1].contains("Unique"));
    }

    #[test]
    fn reorder_column_meta_tracks_drag() {
        let mut meta = vec![
            GridColumnMeta {
                data_type: Some("a".into()),
                ..Default::default()
            },
            GridColumnMeta {
                data_type: Some("b".into()),
                ..Default::default()
            },
            GridColumnMeta {
                data_type: Some("c".into()),
                ..Default::default()
            },
        ];
        reorder_column_meta(&mut meta, 0, 2);
        assert_eq!(
            meta.iter()
                .map(|m| m.data_type.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["b", "a", "c"]
        );
    }
}
