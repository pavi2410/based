//! Read/drive API over the sidebar catalog, used by UI tests.

use gpui_kit::{App, Context, Window};

use super::ConnectionTree;
use super::types::SchemaObject;

/// What the catalog pane currently shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogSnapshot {
    pub selected: Option<usize>,
    /// "Loading objects…", "Not connected", … when no objects are listed.
    pub status: Option<String>,
    pub objects: Vec<SchemaObject>,
}

impl ConnectionTree {
    pub fn catalog(&self, cx: &App) -> CatalogSnapshot {
        let (objects, status) = self
            .browser_list
            .as_ref()
            .map_or_else(Default::default, |l| {
                let d = l.read(cx).delegate();
                (d.visible_objects(), d.status())
            });
        CatalogSnapshot {
            selected: self.selected_connection,
            status,
            objects,
        }
    }

    /// Click the rail icon for connection `idx`.
    pub fn click_connection(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.on_connection_row_clicked(idx, window, cx);
    }

    /// Click the visible object called `name` (as a row click would).
    pub fn click_object(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(object) = self.browser_list.as_ref().and_then(|l| {
            l.read(cx)
                .delegate()
                .visible_objects()
                .into_iter()
                .find(|o| o.display_name() == name)
        }) else {
            return false;
        };
        self.on_object_clicked(object, window, cx);
        true
    }

    /// Filter the catalog as typing in its search box would.
    pub fn search_catalog(&mut self, query: &str, cx: &mut Context<Self>) {
        super::browser_list::apply_catalog_query(self, query, cx);
    }
}
