//! The structure tab: one scrolling spec sheet for a table, shared by the SQL
//! engines through [`InspectSource`]. Columns carry their keys, references and
//! covering indexes; indexes, constraints and incoming references follow; the
//! DDL folds away at the end.

use std::sync::Arc;

use based_core::{ColumnSpec, ObjectRef, TableSpec};
use futures::future::BoxFuture;
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::EditorState,
    menu::PopupMenu,
    skeleton::Skeleton,
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;
use crate::connection::ConnectionId;
use crate::db;
use crate::widgets::empty_state::status_panel;
use crate::widgets::load::LoadFailure;
use crate::widgets::panel::{
    panel_tab_content, tab_breadcrumb_footer, tab_breadcrumb_for_connection,
};
use crate::widgets::sql_editor::{self, new_sql_input};
use crate::workspace::TabSpec;
use crate::workspace::tabs::{inbox, request_workspace_flush};

/// Engine-specific catalog reads for [`TableInspectorPanel`].
pub trait InspectSource: Send + Sync + 'static {
    /// Dock panel name, e.g. `"PgTableInspector"`.
    fn panel_name(&self) -> &'static str;
    /// Breadcrumb id, e.g. `"pg-insp-breadcrumb"`.
    fn breadcrumb_id(&self) -> &'static str;
    /// The table this tab describes.
    fn object(&self) -> ObjectRef;
    fn load(&self) -> BoxFuture<'static, anyhow::Result<TableSpec>>;
}

/// Where the structure stands; the sheet is drawn only from `Ready`.
enum Structure {
    Loading,
    Ready(Box<TableSpec>),
    Failed(LoadFailure),
}

pub struct TableInspectorPanel {
    focus_handle: FocusHandle,
    source: Arc<dyn InspectSource>,
    conn_id: ConnectionId,
    structure: Structure,
    /// Read-only editor for the DDL, made on first render after loading.
    ddl_editor: Option<Entity<EditorState>>,
    ddl_open: bool,
    pub(crate) tab_label: SharedString,
}

impl TableInspectorPanel {
    pub fn new(
        source: Arc<dyn InspectSource>,
        conn_id: ConnectionId,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let tab_label = format!("{} · Structure", source.object().name).into();
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            source,
            conn_id,
            structure: Structure::Loading,
            ddl_editor: None,
            ddl_open: false,
            tab_label,
        };
        panel.load(cx);
        panel
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.structure = Structure::Loading;
        self.ddl_editor = None;
        let fetch = self.source.load();
        cx.spawn(async move |this, cx| {
            let result = db::run(cx, fetch).await;
            let _ = this.update(cx, |panel, cx| {
                panel.structure = match result {
                    Ok(spec) => Structure::Ready(Box::new(spec)),
                    Err(e) => Structure::Failed(LoadFailure::new("Couldn't load the structure", e)),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// The loaded structure, for tests.
    pub fn structure(&self) -> Option<&TableSpec> {
        match &self.structure {
            Structure::Ready(spec) => Some(spec),
            _ => None,
        }
    }

    fn render_sheet(
        &mut self,
        spec: &TableSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.ddl_editor.is_none() && !spec.ddl.is_empty() {
            self.ddl_editor = Some(new_sql_input(&spec.ddl, window, cx));
        }
        let theme = cx.theme().clone();
        let mono = prefs::code_font_family(cx);
        let object = self.source.object();
        let conn = self.conn_id.clone();

        let pk = spec.primary_key().join(", ");
        let facts = h_flex()
            .flex_wrap()
            .gap(px(20.0))
            .text_sm()
            .text_color(theme.muted_foreground)
            .when_some(spec.row_estimate, |h, n| {
                h.child(format!("≈ {} rows", group_digits(n)))
            })
            .when_some(spec.sizes.clone(), |h, s| {
                h.child(format!("{} table · {} indexes", s.table, s.indexes))
            })
            .when(!pk.is_empty(), |h| h.child(format!("Key {pk}")));

        let (open_conn, open_obj) = (conn.clone(), object.clone());
        let ddl_copy = spec.ddl.clone();
        let header = h_flex()
            .flex_wrap()
            .items_end()
            .gap(px(16.0))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(240.0))
                    .gap(px(2.0))
                    .when_some(object.schema.clone(), |v, s| {
                        v.child(
                            div()
                                .text_xs()
                                .font_family(mono.clone())
                                .text_color(theme.muted_foreground)
                                .child(format!("{s}.")),
                        )
                    })
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .font_family(mono.clone())
                            .child(object.name.clone()),
                    )
                    .child(facts),
            )
            .child(
                h_flex()
                    .gap(px(8.0))
                    .child(
                        Button::new("structure-open-data")
                            .small()
                            .outline()
                            .label("Open data")
                            .on_click(move |_, _, cx| {
                                open_tab(
                                    TabSpec::DataViewer {
                                        conn_id: open_conn.clone(),
                                        object: open_obj.clone(),
                                        filter: None,
                                    },
                                    cx,
                                );
                            }),
                    )
                    .when(!ddl_copy.is_empty(), |h| {
                        h.child(
                            Button::new("structure-copy-ddl")
                                .small()
                                .outline()
                                .label("Copy DDL")
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        ddl_copy.clone(),
                                    ));
                                }),
                        )
                    }),
            );

        let columns = self.render_columns(spec, cx);
        let cards = h_flex()
            .flex_wrap()
            .items_start()
            .gap(px(16.0))
            .child(self.render_indexes(spec, cx))
            .child(self.render_constraints(spec, cx))
            .child(self.render_references(spec, cx));

        v_flex()
            .id("structure-sheet")
            .size_full()
            .overflow_y_scroll()
            .px(px(24.0))
            .py(px(20.0))
            .gap(px(24.0))
            .child(header)
            .child(columns)
            .child(cards)
            .children(self.render_ddl(cx))
            .into_any_element()
    }

    fn render_columns(&self, spec: &TableSpec, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mono = prefs::code_font_family(cx);
        // Minimum widths; extra space is shared out, and text never wraps:
        // narrower than this, the table scrolls sideways instead.
        const W: [f32; 5] = [170.0, 190.0, 220.0, 220.0, 200.0];
        let cell = |i: usize| {
            div()
                .flex_1()
                .min_w(px(W[i]))
                .px(px(12.0))
                .py(px(8.0))
                .whitespace_nowrap()
        };
        let head = h_flex()
            .bg(theme.muted.opacity(0.35))
            .text_xs()
            .text_color(theme.muted_foreground)
            .children(
                ["Column", "Type", "Keys", "Default", "Indexed by"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, l)| cell(i).child(l)),
            );
        let rows = spec
            .columns
            .iter()
            .enumerate()
            .map(|(r, c)| {
                let unindexed = spec.is_unindexed_foreign_key(c);
                let indexes = spec.indexes_on(&c.name);
                h_flex()
                    .border_t_1()
                    .border_color(theme.border.opacity(0.6))
                    .text_sm()
                    .font_family(mono.clone())
                    .child(
                        cell(0).child(h_flex().child(c.name.clone()).when(c.nullable, |h| {
                            h.child(div().text_color(theme.muted_foreground).child("?"))
                        })),
                    )
                    .child(
                        cell(1)
                            .text_color(theme.magenta_light)
                            .child(c.data_type.clone()),
                    )
                    .child(cell(2).child(self.keys_cell(r, c, cx)))
                    .child(
                        match c.default.clone() {
                            Some(d) => {
                                let tip = d.clone();
                                cell(3)
                                    .id(("structure-default", r))
                                    .text_color(theme.muted_foreground)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                                    .child(d)
                                    .into_any_element()
                            }
                            None => cell(3)
                                .text_color(theme.muted_foreground)
                                .child("—")
                                .into_any_element(),
                        },
                    )
                    .child(if unindexed {
                        cell(4)
                            .id(("structure-unindexed", r))
                            .font_family(prefs::ui_font_family(cx))
                            .text_color(theme.warning)
                            .tooltip(|window, cx| {
                                Tooltip::new("No index leads with this foreign key, so joins and lookups on it scan the table.")
                                    .build(window, cx)
                            })
                            .child("No index")
                            .into_any_element()
                    } else if indexes.is_empty() {
                        cell(4).text_color(theme.muted_foreground).child("—").into_any_element()
                    } else {
                        cell(4)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(indexes.join(", "))
                            .into_any_element()
                    })
            })
            .collect::<Vec<_>>();
        v_flex()
            .gap(px(8.0))
            .child(section_title(
                format!("Columns · {}", spec.columns.len()),
                cx,
            ))
            .child(
                div()
                    .id("structure-columns")
                    .overflow_x_scroll()
                    .border_1()
                    .border_color(theme.border)
                    .rounded(px(8.0))
                    .child(
                        v_flex()
                            .min_w(px(W.iter().sum::<f32>()))
                            .child(head)
                            .children(rows),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("? marks a column that can be NULL."),
            )
            .into_any_element()
    }

    fn keys_cell(&self, row: usize, c: &ColumnSpec, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let badge = |label: &'static str, tone: Hsla| {
            div()
                .px(px(6.0))
                .rounded_full()
                .border_1()
                .border_color(tone.opacity(0.5))
                .bg(tone.opacity(0.12))
                .text_color(tone)
                .text_xs()
                .child(label)
        };
        let mut keys = h_flex().gap(px(6.0)).items_center();
        if c.primary_key {
            keys = keys.child(badge("PK", theme.warning));
        }
        if let Some(target) = c.references.clone() {
            let conn = self.conn_id.clone();
            let label = format!("→ {}.{}", target.table.name, target.column);
            keys = keys.child(badge("FK", theme.blue_light)).child(
                Button::new(("structure-fk", row))
                    .ghost()
                    .xsmall()
                    .label(label)
                    .tooltip("Open its structure")
                    .on_click(move |_, _, cx| {
                        open_tab(
                            TabSpec::Inspector {
                                conn_id: conn.clone(),
                                object: target.table.clone(),
                            },
                            cx,
                        );
                    }),
            );
        }
        keys.into_any_element()
    }

    fn render_indexes(&self, spec: &TableSpec, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mono = prefs::code_font_family(cx);
        let items = spec.indexes.iter().map(|i| {
            let mut detail = Vec::new();
            if i.unique {
                detail.push("unique".to_owned());
            }
            if let Some(m) = &i.method {
                detail.push(m.clone());
            }
            v_flex()
                .gap(px(2.0))
                .child(div().font_family(mono.clone()).child(i.name.clone()))
                .child(
                    div().text_color(theme.muted_foreground).child(
                        format!("{} ({})", detail.join(" · "), i.columns.join(", "))
                            .trim()
                            .to_owned(),
                    ),
                )
        });
        card(
            format!("Indexes · {}", spec.indexes.len()),
            items.map(IntoElement::into_any_element).collect(),
            cx,
        )
    }

    fn render_constraints(&self, spec: &TableSpec, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mono = prefs::code_font_family(cx);
        let items = spec.constraints.iter().map(|c| {
            v_flex()
                .gap(px(2.0))
                .child(div().font_family(mono.clone()).child(c.name.clone()))
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(c.definition.clone()),
                )
                .into_any_element()
        });
        card(
            format!("Constraints · {}", spec.constraints.len()),
            items.collect(),
            cx,
        )
    }

    fn render_references(&self, spec: &TableSpec, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let this = self.source.object();
        let items = spec.referenced_by.iter().enumerate().map(|(i, r)| {
            let conn = self.conn_id.clone();
            let table = r.from_table.clone();
            v_flex()
                .items_start()
                .gap(px(2.0))
                .child(
                    Button::new(("structure-ref", i))
                        .ghost()
                        .xsmall()
                        .label(format!(
                            "{}.{}",
                            r.from_table.name,
                            r.from_columns.join(", ")
                        ))
                        .tooltip("Open its structure")
                        .on_click(move |_, _, cx| {
                            open_tab(
                                TabSpec::Inspector {
                                    conn_id: conn.clone(),
                                    object: table.clone(),
                                },
                                cx,
                            );
                        }),
                )
                .child(div().text_color(theme.muted_foreground).child(format!(
                    "→ {}.{}",
                    this.name,
                    r.to_columns.join(", ")
                )))
                .into_any_element()
        });
        card(
            format!("Referenced by · {}", spec.referenced_by.len()),
            items.collect(),
            cx,
        )
    }

    fn render_ddl(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let editor = self.ddl_editor.clone()?;
        let theme = cx.theme().clone();
        let open = self.ddl_open;
        Some(
            v_flex()
                .border_1()
                .border_color(theme.border)
                .rounded(px(8.0))
                .child(
                    h_flex().child(
                        Button::new("structure-ddl-toggle")
                            .ghost()
                            .small()
                            .icon(if open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .label("DDL")
                            .on_click(cx.listener(|p, _, _, cx| {
                                p.ddl_open = !p.ddl_open;
                                cx.notify();
                            })),
                    ),
                )
                .when(open, |v| {
                    v.child(
                        div()
                            .h(px(280.0))
                            .border_t_1()
                            .border_color(theme.border)
                            .child(sql_editor::code_editor_flex(&editor, false, true, cx)),
                    )
                })
                .into_any_element(),
        )
    }
}

/// Open `spec` as a tab (another table's structure, this table's data).
fn open_tab(spec: TabSpec, cx: &mut App) {
    inbox::open_tab(spec, cx);
    request_workspace_flush(cx);
}

fn section_title(text: String, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

/// A titled card listing `items`, or "None".
fn card(title: String, items: Vec<AnyElement>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let empty = items.is_empty();
    v_flex()
        .flex_1()
        .min_w(px(280.0))
        .gap(px(8.0))
        .child(section_title(title, cx))
        .child(
            v_flex()
                .p(px(12.0))
                .gap(px(10.0))
                .border_1()
                .border_color(theme.border)
                .rounded(px(8.0))
                .text_sm()
                .when(empty, |v| {
                    v.child(div().text_color(theme.muted_foreground).child("None"))
                })
                .children(items),
        )
        .into_any_element()
}

/// `5000` → `5,000`.
fn group_digits(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}

impl EventEmitter<PanelEvent> for TableInspectorPanel {}

impl Focusable for TableInspectorPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for TableInspectorPanel {
    fn panel_name(&self) -> &'static str {
        self.source.panel_name()
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for TableInspectorPanel {
    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    crate::based_panel_tab_chrome!();
}

impl Render for TableInspectorPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.structure {
            Structure::Loading => v_flex()
                .p(px(24.0))
                .gap(px(12.0))
                .child(Skeleton::new().w(px(220.0)).h(px(24.0)).rounded(px(4.0)))
                .children((0..6).map(|_| Skeleton::new().h(px(14.0)).rounded(px(4.0))))
                .into_any_element(),
            Structure::Failed(LoadFailure { title, detail }) => {
                let copy = detail.to_string();
                status_panel(
                    title,
                    "Check the connection and your access to this object, then try again.",
                    IconName::TriangleAlert,
                    Some(cx.theme().danger),
                    Some(detail.clone()),
                    vec![
                        Button::new("inspector-retry")
                            .small()
                            .primary()
                            .label("Retry")
                            .on_click(cx.listener(|p, _, _, cx| p.load(cx)))
                            .into_any_element(),
                        Button::new("inspector-copy-error")
                            .small()
                            .outline()
                            .label("Copy details")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                            })
                            .into_any_element(),
                    ],
                    cx,
                )
            }
            Structure::Ready(spec) => {
                let spec = spec.clone();
                self.render_sheet(&spec, window, cx)
            }
        };
        let object = self.source.object();
        let path: Vec<String> = object.schema.iter().cloned().chain([object.name]).collect();
        let crumbs = tab_breadcrumb_for_connection(&self.conn_id, path, cx);
        let footer = tab_breadcrumb_footer(self.source.breadcrumb_id(), crumbs, None, cx);
        panel_tab_content(body, footer).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::group_digits;

    #[test]
    fn groups_thousands() {
        assert_eq!(group_digits(5000), "5,000");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1_234_567), "1,234,567");
    }
}
