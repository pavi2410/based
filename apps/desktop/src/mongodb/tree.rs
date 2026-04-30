// mongodb::tree — collection list for a database.

use crate::widgets::ui::{metadata_pill, panel_header};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    v_flex,
};
use mongodb::Database;

pub enum CollectionTreeEvent {
    CollectionSelected(String),
}

pub struct CollectionsTreePanel {
    focus_handle: FocusHandle,
    database: Database,
    names: Vec<String>,
    selected: Option<String>,
}

impl CollectionsTreePanel {
    pub fn new(database: Database, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut p = Self {
            focus_handle: cx.focus_handle(),
            database,
            names: vec![],
            selected: None,
        };
        p.reload(cx);
        p
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let db = self.database.clone();
        cx.spawn(async move |this, cx| {
            let names = match crate::db::run_infallible(cx, async move {
                db.list_collection_names(None).await.unwrap_or_default()
            })
            .await
            {
                Ok(n) => n,
                Err(_) => return,
            };
            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.names = names;
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for CollectionsTreePanel {}
impl EventEmitter<CollectionTreeEvent> for CollectionsTreePanel {}

impl Focusable for CollectionsTreePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for CollectionsTreePanel {
    fn panel_name(&self) -> &'static str {
        "MongoCollectionsTree"
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Collections"
    }
}

impl Render for CollectionsTreePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let fg = cx.theme().foreground;
        let names = self.names.clone();
        let selected = self.selected.clone();

        v_flex()
            .id("mongo-tree")
            .size_full()
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .child(panel_header(
                "MongoDB Objects",
                "Collections, documents, indexes, and streams",
                cx,
            ))
            .child(
                h_flex()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .border_b_1()
                    .border_color(border.opacity(0.72))
                    .bg(cx.theme().muted.opacity(0.18))
                    .child(metadata_pill("collections", names.len().to_string(), cx))
                    .child(metadata_pill("engine", "MongoDB", cx)),
            )
            .children(names.into_iter().enumerate().map(|(ix, name)| {
                let sel = selected.as_ref() == Some(&name);
                let n = name.clone();
                h_flex()
                    .id(("coll", ix))
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .when(sel, |d| d.bg(cx.theme().accent.opacity(0.12)))
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        panel.selected = Some(n.clone());
                        cx.emit(CollectionTreeEvent::CollectionSelected(n.clone()));
                        cx.notify();
                    }))
                    .child(div().text_sm().text_color(fg).child(name))
            }))
    }
}
