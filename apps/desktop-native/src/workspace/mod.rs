// workspace/ — engine-agnostic Workspace entity, Pane, Item trait,
// Sidebar, StatusBar, TopBar, and the Welcome view.
// Implemented in Phase 1.

pub mod item;
pub mod pane;
pub mod sidebar;
pub mod status_bar;
pub mod topbar;
pub mod welcome;

use std::time::Instant;

use gpui::{Context, Entity, FocusHandle, Focusable, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme,
    dock::{DockArea, DockItem, PanelStyle},
    h_flex, v_flex,
};

use crate::connection::{ConnectionConfig, ConnectionEntry, ConnectionState};
use crate::mongodb::MongoConfig;
use crate::postgres::{PostgresConfig, SslMode};
use crate::sqlite::SqliteConfig;
use sidebar::ConnectionSidebar;
use status_bar::StatusBar;
use topbar::Topbar;
use welcome::WelcomePanel;

pub struct Workspace {
    dock_area: Entity<DockArea>,
    mock_connections: Vec<ConnectionEntry>,
    #[allow(dead_code)]
    sidebar_collapsed: bool,
    focus_handle: FocusHandle,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mock_connections = Self::make_mock_connections();

        let dock_area = cx.new(|cx| {
            DockArea::new("workspace", Some(1), window, cx).panel_style(PanelStyle::TabBar)
        });

        let welcome = cx.new(|cx| WelcomePanel::new(window, cx));
        let center = DockItem::tab(welcome, &dock_area.downgrade(), window, cx);
        dock_area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
        });

        Self {
            dock_area,
            mock_connections,
            sidebar_collapsed: false,
            focus_handle: cx.focus_handle(),
        }
    }

    fn make_mock_connections() -> Vec<ConnectionEntry> {
        let mut entries = Vec::new();

        entries.push(ConnectionEntry::new(ConnectionConfig::Postgres(
            PostgresConfig {
                label: "prod-postgres".to_string(),
                host: "localhost".to_string(),
                port: 5432,
                database: "prod".to_string(),
                username: "admin".to_string(),
                password: String::new(),
                ssl_mode: SslMode::Prefer,
            },
        )));

        entries.push(ConnectionEntry::new(ConnectionConfig::Postgres(
            PostgresConfig {
                label: "staging-postgres".to_string(),
                host: "staging.example.com".to_string(),
                port: 5432,
                database: "staging".to_string(),
                username: "app".to_string(),
                password: String::new(),
                ssl_mode: SslMode::Require,
            },
        )));

        let mut mongo = ConnectionEntry::new(ConnectionConfig::MongoDB(MongoConfig {
            label: "local-mongo".to_string(),
            uri: "mongodb://localhost:27017".to_string(),
            database: None,
            auth_source: None,
        }));
        mongo.state = ConnectionState::Failed {
            reason: "auth failed".to_string(),
            attempted_at: Instant::now(),
        };
        entries.push(mongo);

        entries.push(ConnectionEntry::new(ConnectionConfig::SQLite(SqliteConfig {
            label: "app.db".to_string(),
            path: "app.db".into(),
            wal: true,
        })));

        entries
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let conn_count = self.mock_connections.len();
        // Clone the connections so ConnectionSidebar can own them
        let connections: Vec<ConnectionEntry> = self
            .mock_connections
            .iter()
            .map(|e| ConnectionEntry {
                id: e.id.clone(),
                config: e.config.clone(),
                state: ConnectionState::Disconnected,
                last_connected_at: e.last_connected_at,
                last_error: e.last_error.clone(),
            })
            .collect();

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(Topbar::new("No Project"))
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(ConnectionSidebar::new(connections))
                    .child(
                        div()
                            .flex_1()
                            .size_full()
                            .overflow_hidden()
                            .child(self.dock_area.clone()),
                    ),
            )
            .child(StatusBar::new(conn_count))
    }
}
