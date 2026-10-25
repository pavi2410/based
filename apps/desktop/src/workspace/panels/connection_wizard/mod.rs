//! Unified New connection form: engine control, Test/Connect, then Name/Save.

use std::path::PathBuf;

use gpui_kit::component::{
    ActiveTheme, Disableable as _, IndexPath, Sizable as _, Size, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::{DialogAction, DialogClose, DialogFooter},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::{Input, InputContentType, InputEvent, InputState},
    menu::PopupMenu,
    notification::Notification,
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectState},
    separator::Separator,
    switch::Switch,
    tag::Tag,
    v_flex,
};
use gpui_kit::{prelude::*, *};
use tokio::task::spawn_blocking;
use uuid::Uuid;

use based_core::SshTunnelConfig;
use based_project::AiSettings;

use crate::app::prefs;
use crate::connection::{
    ConnectionConfig, ConnectionEntry, ConnectionId, EngineKind, OpenedConnection,
    categorize_connect_error,
    lifecycle::{Connectable, TestReport},
    open_connection,
};
use crate::db;
use crate::mongodb::{MongoConfig, MongoConnection};
use crate::postgres::wizard::parse_postgres_uri;
use crate::postgres::{PgConnection, PostgresConfig, SslMode};
use crate::project::ProjectRoot;
use crate::redis::{RedisConfig, RedisConnection};
use crate::sqlite::{SqliteConfig, SqliteConnection, SqlitePragma};
use crate::widgets::{labeled_field, labeled_fixed, new_field, set_field};
use crate::workspace::WorkspaceRef;
use crate::workspace::connection_destination::{
    ConnectionDestination, destination_row, resolve_wizard_destination,
};
use crate::workspace::wizard_logic::{
    AI_LABELS, add_wizard_tag, ai_from_label, ai_label, can_edit_saved_connection,
    remove_wizard_tag, save_label_from_config, ssl_mode_from_toggle, ssl_toggle_enabled,
    wizard_engine_label, wizard_session_id,
};

mod actions;
mod config;
mod fields;
mod view;

const ENGINE_LABELS: &[&str] = &["PostgreSQL", "MongoDB", "SQLite", "Redis"];
const SSL_ON_LABELS: &[&str] = &["require", "verify-ca", "verify-full"];
const WIZARD_COLUMN_W: f32 = 480.0;

pub enum WizardStatus {
    Idle,
    Testing,
    TestOk { latency_ms: u64, detail: String },
    TestErr(String),
    Connecting,
    ConnectOk,
    ConnectErr(String),
    Saving,
    Saved,
    SaveErr(String),
}

pub struct ConnectionWizardPanel {
    focus_handle: FocusHandle,
    engine: Entity<SelectState<Vec<&'static str>>>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    database: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    ssl_enabled: bool,
    ssl_mode: Entity<SelectState<Vec<&'static str>>>,
    ssh_enabled: bool,
    ssh_host: Entity<InputState>,
    ssh_port: Entity<InputState>,
    ssh_user: Entity<InputState>,
    ssh_key_path: Entity<InputState>,
    ssh_key_passphrase: Entity<InputState>,
    uri: Entity<InputState>,
    mongo_uri: Entity<InputState>,
    mongo_database: Entity<InputState>,
    redis_url: Entity<InputState>,
    sqlite_path: Entity<InputState>,
    sqlite_read_only: bool,
    sqlite_pragma: Option<SqlitePragma>,
    name: Entity<InputState>,
    tag_input: Entity<InputState>,
    tags: Vec<String>,
    ai_select: Entity<SelectState<Vec<&'static str>>>,
    destination: Option<ConnectionDestination>,
    session_id: Option<ConnectionId>,
    status: WizardStatus,
    pub(crate) tab_label: SharedString,
}

impl ConnectionWizardPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let uri = new_field(window, cx, "", "postgresql://user:pass@host:5432/db");
        cx.subscribe_in(&uri, window, |panel, _, event, window, cx| {
            if let InputEvent::PressEnter {
                secondary: false,
                shift: false,
            } = event
            {
                panel.apply_uri(window, cx);
                cx.notify();
            }
        })
        .detach();

        let engine = cx.new(|cx| {
            SelectState::new(ENGINE_LABELS.to_vec(), Some(IndexPath::new(0)), window, cx)
        });
        cx.subscribe_in(&engine, window, |panel, _, _: &SelectEvent<_>, _, cx| {
            panel.status = WizardStatus::Idle;
            cx.notify();
        })
        .detach();

        let tag_input = new_field(window, cx, "", "Add a tag");
        cx.subscribe_in(&tag_input, window, |panel, _, event, window, cx| {
            if let InputEvent::PressEnter {
                secondary: false,
                shift: false,
            } = event
            {
                panel.add_tag_from_input(window, cx);
                cx.notify();
            }
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            engine,
            host: new_field(window, cx, "localhost", "Host"),
            port: new_field(window, cx, "5432", "Port"),
            database: new_field(window, cx, "postgres", "Database"),
            username: new_field(window, cx, "postgres", "Username"),
            password: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Password")
                    .masked(true)
            }),
            ssl_enabled: false,
            ssl_mode: cx.new(|cx| {
                SelectState::new(SSL_ON_LABELS.to_vec(), Some(IndexPath::new(0)), window, cx)
            }),
            ssh_enabled: false,
            ssh_host: new_field(window, cx, "", "bastion.example.com"),
            ssh_port: new_field(window, cx, "22", "22"),
            ssh_user: new_field(window, cx, "", "ec2-user"),
            ssh_key_path: new_field(window, cx, "", "~/.ssh/id_ed25519"),
            ssh_key_passphrase: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Key passphrase (optional)")
                    .masked(true)
            }),
            uri,
            mongo_uri: new_field(
                window,
                cx,
                "mongodb://127.0.0.1:27017",
                "mongodb://host:27017",
            ),
            mongo_database: new_field(window, cx, "", "Database (optional)"),
            redis_url: new_field(
                window,
                cx,
                "redis://127.0.0.1:6379",
                "redis://[:password@]host:6379[/db]",
            ),
            sqlite_path: new_field(window, cx, "", "/path/to/database.db"),
            sqlite_read_only: false,
            sqlite_pragma: None,
            name: new_field(window, cx, "", "Name"),
            tag_input,
            tags: Vec::new(),
            destination: None,
            session_id: None,
            ai_select: cx.new(|cx| {
                SelectState::new(AI_LABELS.to_vec(), Some(IndexPath::new(0)), window, cx)
            }),
            status: WizardStatus::Idle,
            tab_label: "New connection".into(),
        }
    }

    pub fn edit(
        entry: Entity<ConnectionEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (id, config, tags, origin, ai) = {
            let e = entry.read(cx);
            (
                e.id.clone(),
                e.config.clone(),
                e.tags.clone(),
                e.origin,
                e.ai,
            )
        };
        let mut panel = Self::new(window, cx);
        panel.session_id = Some(id);
        panel.destination = Some(ConnectionDestination::from_origin(origin));
        panel.tags = tags;
        panel.ai_select.update(cx, |select, cx| {
            select.set_selected_value(&ai_label(ai), window, cx);
        });
        panel.tab_label = config.label().to_string().into();
        panel.apply_config(&config, window, cx);
        panel
    }
}

impl EventEmitter<PanelEvent> for ConnectionWizardPanel {}

impl Focusable for ConnectionWizardPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for ConnectionWizardPanel {
    crate::based_panel_behavior!("ConnectionWizard");
}

impl Panel for ConnectionWizardPanel {
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
