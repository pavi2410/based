// mongodb::tree — collection list for a database.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
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
            }).await {
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
            .child(
                div()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(border)
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Collections"),
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
