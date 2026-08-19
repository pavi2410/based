//! Unified New connection form: engine control, Test/Connect, then Name/Save.

use std::path::PathBuf;

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable as _, IndexPath, Sizable as _, Size, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::{DialogAction, DialogClose, DialogFooter},
    dock::{Panel, PanelEvent},
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
use tokio::task::spawn_blocking;
use uuid::Uuid;

use based_core::SshTunnelConfig;

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
use crate::sqlite::{SqliteConfig, SqliteConnection, SqlitePragma};
use crate::widgets::{labeled_field, labeled_fixed, new_field, set_field};
use crate::workspace::WorkspaceRef;
use crate::workspace::connection_destination::{
    ConnectionDestination, destination_row, resolve_wizard_destination,
};
use crate::workspace::wizard_logic::{
    add_wizard_tag, can_edit_saved_connection, remove_wizard_tag, save_label_from_config,
    ssl_mode_from_toggle, ssl_toggle_enabled, wizard_engine_label, wizard_session_id,
};

const ENGINE_LABELS: &[&str] = &["PostgreSQL", "MongoDB", "SQLite"];
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
    sqlite_path: Entity<InputState>,
    sqlite_read_only: bool,
    sqlite_pragma: Option<SqlitePragma>,
    name: Entity<InputState>,
    tag_input: Entity<InputState>,
    tags: Vec<String>,
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
            sqlite_path: new_field(window, cx, "", "/path/to/database.db"),
            sqlite_read_only: false,
            sqlite_pragma: None,
            name: new_field(window, cx, "", "Name"),
            tag_input,
            tags: Vec::new(),
            destination: None,
            session_id: None,
            status: WizardStatus::Idle,
            tab_label: "New connection".into(),
        }
    }

    pub fn edit(
        entry: Entity<ConnectionEntry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (id, config, tags, origin) = {
            let e = entry.read(cx);
            (e.id.clone(), e.config.clone(), e.tags.clone(), e.origin)
        };
        let mut panel = Self::new(window, cx);
        panel.session_id = Some(id);
        panel.destination = Some(ConnectionDestination::from_origin(origin));
        panel.tags = tags;
        panel.tab_label = config.label().to_string().into();
        panel.apply_config(&config, window, cx);
        panel
    }

    pub fn editing_id(&self) -> Option<&ConnectionId> {
        self.session_id
            .as_ref()
            .filter(|id| can_edit_saved_connection(id))
    }

    fn is_edit(&self) -> bool {
        self.editing_id().is_some()
    }

    fn apply_config(
        &mut self,
        config: &ConnectionConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let engine_label = wizard_engine_label(config.engine());
        self.engine.update(cx, |select, cx| {
            select.set_selected_value(&engine_label, window, cx);
        });
        set_field(&self.name, config.label(), window, cx);
        match config {
            ConnectionConfig::Postgres(c) => {
                set_field(&self.host, &c.host, window, cx);
                set_field(&self.port, &c.port.to_string(), window, cx);
                set_field(&self.database, &c.database, window, cx);
                set_field(&self.username, &c.username, window, cx);
                set_field(&self.password, &c.password, window, cx);
                self.ssl_enabled = ssl_toggle_enabled(c.ssl_mode);
                if self.ssl_enabled {
                    let label = ssl_on_label(c.ssl_mode);
                    self.ssl_mode.update(cx, |select, cx| {
                        select.set_selected_value(&label, window, cx);
                    });
                }
                self.apply_ssh(c.ssh.as_ref(), window, cx);
            }
            ConnectionConfig::MongoDB(c) => {
                set_field(&self.mongo_uri, &c.uri, window, cx);
                set_field(
                    &self.mongo_database,
                    c.database.as_deref().unwrap_or(""),
                    window,
                    cx,
                );
            }
            ConnectionConfig::SQLite(c) => {
                set_field(&self.sqlite_path, &c.path.to_string_lossy(), window, cx);
                self.sqlite_read_only = c.read_only;
                self.sqlite_pragma = c.pragma.clone();
            }
        }
    }

    fn current_engine(&self, cx: &App) -> EngineKind {
        match self.engine.read(cx).selected_value().copied() {
            Some("MongoDB") => EngineKind::MongoDB,
            Some("SQLite") => EngineKind::SQLite,
            _ => EngineKind::Postgres,
        }
    }

    fn apply_ssh(
        &mut self,
        ssh: Option<&SshTunnelConfig>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ssh {
            Some(ssh) => {
                self.ssh_enabled = true;
                set_field(&self.ssh_host, &ssh.host, window, cx);
                set_field(&self.ssh_port, &ssh.port.to_string(), window, cx);
                set_field(&self.ssh_user, &ssh.user, window, cx);
                set_field(
                    &self.ssh_key_path,
                    ssh.key_path.as_deref().unwrap_or(""),
                    window,
                    cx,
                );
                set_field(
                    &self.ssh_key_passphrase,
                    ssh.key_passphrase.as_deref().unwrap_or(""),
                    window,
                    cx,
                );
            }
            None => {
                self.ssh_enabled = false;
            }
        }
    }

    fn current_ssh(&self, cx: &App) -> Option<SshTunnelConfig> {
        if !self.ssh_enabled {
            return None;
        }
        let key_path = self.ssh_key_path.read(cx).value().to_string();
        let key_passphrase = self.ssh_key_passphrase.read(cx).value().to_string();
        Some(SshTunnelConfig {
            host: self.ssh_host.read(cx).value().to_string(),
            port: self.ssh_port.read(cx).value().parse().unwrap_or(22),
            user: self.ssh_user.read(cx).value().to_string(),
            key_path: nonempty_opt(&key_path),
            key_passphrase: nonempty_opt(&key_passphrase),
        })
    }

    fn current_ssl_mode(&self, cx: &App) -> SslMode {
        let selected = self
            .ssl_mode
            .read(cx)
            .selected_value()
            .copied()
            .and_then(ssl_mode_from_on_label);
        ssl_mode_from_toggle(self.ssl_enabled, selected)
    }

    fn config(&self, cx: &App) -> ConnectionConfig {
        match self.current_engine(cx) {
            EngineKind::Postgres => ConnectionConfig::Postgres(PostgresConfig {
                label: self.name.read(cx).value().to_string(),
                host: self.host.read(cx).value().to_string(),
                port: self.port.read(cx).value().parse().unwrap_or(5432u16),
                database: self.database.read(cx).value().to_string(),
                username: self.username.read(cx).value().to_string(),
                password: self.password.read(cx).value().to_string(),
                ssl_mode: self.current_ssl_mode(cx),
                ssh: self.current_ssh(cx),
            }),
            EngineKind::MongoDB => {
                let database = self.mongo_database.read(cx).value().to_string();
                ConnectionConfig::MongoDB(MongoConfig {
                    label: self.name.read(cx).value().to_string(),
                    uri: self.mongo_uri.read(cx).value().to_string(),
                    database: nonempty_opt(&database),
                    auth_source: None,
                })
            }
            EngineKind::SQLite => ConnectionConfig::SQLite(SqliteConfig {
                label: self.name.read(cx).value().to_string(),
                path: PathBuf::from(self.sqlite_path.read(cx).value().as_ref()),
                read_only: self.sqlite_read_only,
                pragma: self.sqlite_pragma.clone(),
            }),
        }
    }

    fn labeled_config(&self, cx: &App) -> ConnectionConfig {
        let config = self.config(cx);
        let label = save_label_from_config(self.name.read(cx).value().as_ref(), &config);
        config.with_label(label)
    }

    fn apply_uri(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let uri = self.uri.read(cx).value().to_string();
        let Some(cfg) = parse_postgres_uri(&uri) else {
            self.status = WizardStatus::TestErr("Could not parse URI".into());
            return false;
        };
        set_field(&self.host, &cfg.host, window, cx);
        set_field(&self.port, &cfg.port.to_string(), window, cx);
        set_field(&self.database, &cfg.database, window, cx);
        set_field(&self.username, &cfg.username, window, cx);
        set_field(&self.password, &cfg.password, window, cx);
        self.ssl_enabled = ssl_toggle_enabled(cfg.ssl_mode);
        if self.ssl_enabled {
            let label = ssl_on_label(cfg.ssl_mode);
            self.ssl_mode.update(cx, |select, cx| {
                select.set_selected_value(&label, window, cx);
            });
        }
        self.status = WizardStatus::Idle;
        true
    }

    fn open_import_url_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let uri = self.uri.clone();
        let panel = cx.entity().downgrade();
        let code_font = prefs::code_font_family(cx);
        window.open_dialog(cx, move |dialog, _, _| {
            let uri = uri.clone();
            let panel = panel.clone();
            dialog
                .title("Import from URL")
                .child(
                    Input::new(&uri)
                        .content_type(InputContentType::Url)
                        .cleanable(true)
                        .aria_label("Connection URI")
                        .font_family(code_font.clone())
                        .w_full(),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogClose::new()
                                .child(Button::new("import-url-cancel").outline().label("Cancel")),
                        )
                        .child(
                            DialogAction::new()
                                .child(Button::new("import-url-apply").primary().label("Apply")),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    let mut applied = false;
                    let _ = panel.update(cx, |panel, cx| {
                        applied = panel.apply_uri(window, cx);
                        cx.notify();
                    });
                    if !applied {
                        window.push_notification(
                            Notification::error("Could not parse URI").title("Import from URL"),
                            cx,
                        );
                    }
                    applied
                })
        });
    }

    fn add_tag_from_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.tag_input.read(cx).value().to_string();
        if add_wizard_tag(&mut self.tags, &raw) {
            set_field(&self.tag_input, "", window, cx);
        }
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Testing;
        let config = self.config(cx);
        let task = match &config {
            ConnectionConfig::Postgres(cfg) => PgConnection::test(cfg, cx),
            ConnectionConfig::MongoDB(cfg) => MongoConnection::test(cfg, cx),
            ConnectionConfig::SQLite(cfg) => SqliteConnection::test(cfg, cx),
        };
        cx.spawn(async move |this, cx| {
            let result = task.await;
            cx.update(|cx| apply_test_result(this, result, cx));
        })
        .detach();
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.status = WizardStatus::Connecting;
        let config = self.labeled_config(cx);
        let tags = self.tags.clone();
        let key = Uuid::new_v4().to_string();
        let session_id = wizard_session_id(self.session_id.as_ref(), &key);
        self.session_id = Some(session_id.clone());
        let task = open_connection(config.clone(), cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|_window, cx| match result {
                Ok(opened) => {
                    if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                        ws.update(cx, |workspace, cx| {
                            workspace
                                .attach_wizard_session(config, opened, session_id, tags, true, cx);
                        });
                        let _ = this.update(cx, |panel, cx| {
                            panel.status = WizardStatus::ConnectOk;
                            cx.notify();
                        });
                    } else {
                        drop_opened(opened);
                        let _ = this.update(cx, |panel, cx| {
                            panel.status = WizardStatus::ConnectErr("Workspace is not open".into());
                            cx.notify();
                        });
                    }
                }
                Err(e) => {
                    let _ = this.update(cx, |panel, cx| {
                        panel.status = WizardStatus::ConnectErr(
                            categorize_connect_error(&e.to_string()).display_message(),
                        );
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let destination =
            match resolve_wizard_destination(cx.has_global::<ProjectRoot>(), self.destination) {
                Ok(dest) => dest,
                Err(msg) => {
                    self.status = WizardStatus::SaveErr(msg);
                    cx.notify();
                    return;
                }
            };
        let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) else {
            self.status = WizardStatus::SaveErr("Workspace is not open".into());
            cx.notify();
            return;
        };
        self.status = WizardStatus::Saving;
        let config = self.labeled_config(cx);
        let session_id = self.session_id.clone();
        let result = ws.update(cx, |workspace, cx| {
            workspace.save_wizard_connection(config, destination, session_id, self.tags.clone(), cx)
        });
        match result {
            Ok((id, reconnect)) => {
                self.session_id = Some(id);
                self.tab_label = self.labeled_config(cx).label().to_string().into();
                if reconnect {
                    self.reconnect_saved(window, cx);
                } else {
                    self.status = WizardStatus::Saved;
                }
            }
            Err(msg) => self.status = WizardStatus::SaveErr(msg),
        }
        cx.notify();
    }

    fn reconnect_saved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session_id) = self.session_id.clone() else {
            self.status = WizardStatus::Saved;
            return;
        };
        self.status = WizardStatus::Connecting;
        let config = self.labeled_config(cx);
        let tags = self.tags.clone();
        let task = open_connection(config.clone(), cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|_window, cx| match result {
                Ok(opened) => {
                    if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                        ws.update(cx, |workspace, cx| {
                            workspace
                                .attach_wizard_session(config, opened, session_id, tags, false, cx);
                        });
                        let _ = this.update(cx, |panel, cx| {
                            panel.status = WizardStatus::Saved;
                            cx.notify();
                        });
                    } else {
                        drop_opened(opened);
                        let _ = this.update(cx, |panel, cx| {
                            panel.status = WizardStatus::SaveErr("Workspace is not open".into());
                            cx.notify();
                        });
                    }
                }
                Err(e) => {
                    let msg = categorize_connect_error(&e.to_string()).display_message();
                    if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                        ws.update(cx, |workspace, cx| {
                            workspace.fail_reconnect(&session_id, msg.clone(), cx);
                        });
                    }
                    let _ = this.update(cx, |panel, cx| {
                        panel.status =
                            WizardStatus::SaveErr(format!("Saved, but reconnect failed: {msg}"));
                        cx.notify();
                    });
                }
            });
        })
        .detach();
    }

    fn browse_sqlite(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path_field = self.sqlite_path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let picked = db::run_infallible(cx, async {
                spawn_blocking(|| {
                    rfd::FileDialog::new()
                        .set_title("Choose SQLite database")
                        .add_filter("SQLite", &["db", "sqlite", "sqlite3"])
                        .pick_file()
                })
                .await
                .ok()
                .flatten()
            })
            .await
            .ok()
            .flatten();

            let Some(path) = picked else {
                return;
            };
            let _ = cx.update(|window, cx| {
                this.update(cx, |panel, cx| {
                    set_field(&path_field, &path.display().to_string(), window, cx);
                    panel.status = WizardStatus::Idle;
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn browse_ssh_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path_field = self.ssh_key_path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let picked = db::run_infallible(cx, async {
                spawn_blocking(|| {
                    rfd::FileDialog::new()
                        .set_title("Choose SSH private key")
                        .pick_file()
                })
                .await
                .ok()
                .flatten()
            })
            .await
            .ok()
            .flatten();

            let Some(path) = picked else {
                return;
            };
            let _ = cx.update(|window, cx| {
                this.update(cx, |panel, cx| {
                    set_field(&path_field, &path.display().to_string(), window, cx);
                    panel.status = WizardStatus::Idle;
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn busy(&self) -> bool {
        matches!(
            self.status,
            WizardStatus::Testing | WizardStatus::Connecting | WizardStatus::Saving
        )
    }
}

fn apply_test_result(
    this: gpui::WeakEntity<ConnectionWizardPanel>,
    result: anyhow::Result<TestReport>,
    cx: &mut App,
) {
    let _ = this.update(cx, |panel, cx| {
        panel.status = match result {
            Ok(report) => WizardStatus::TestOk {
                latency_ms: report.latency_ms,
                detail: report.server_version.or(report.message).unwrap_or_default(),
            },
            Err(e) => {
                WizardStatus::TestErr(categorize_connect_error(&e.to_string()).display_message())
            }
        };
        cx.notify();
    });
}

fn drop_opened(opened: OpenedConnection) {
    match opened {
        OpenedConnection::Postgres(conn) => db::close_pg_pool(conn.pool),
        OpenedConnection::Sqlite(conn) => db::close_sqlite_pool(conn.pool),
        OpenedConnection::MongoDB(_) => {}
    }
}

fn nonempty_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn ssl_on_label(mode: SslMode) -> &'static str {
    match mode {
        SslMode::VerifyCa => "verify-ca",
        SslMode::VerifyFull => "verify-full",
        _ => "require",
    }
}

fn ssl_mode_from_on_label(label: &str) -> Option<SslMode> {
    Some(match label {
        "require" => SslMode::Require,
        "verify-ca" => SslMode::VerifyCa,
        "verify-full" => SslMode::VerifyFull,
        _ => return None,
    })
}

impl EventEmitter<PanelEvent> for ConnectionWizardPanel {}

impl Focusable for ConnectionWizardPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ConnectionWizardPanel {
    fn panel_name(&self) -> &'static str {
        "ConnectionWizard"
    }

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

impl Render for ConnectionWizardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let engine = self.current_engine(cx);
        let busy = self.busy();
        let editing = self.is_edit();
        let heading: SharedString = if editing {
            "Edit connection".into()
        } else {
            "New connection".into()
        };
        let status: SharedString = match &self.status {
            WizardStatus::Idle => "".into(),
            WizardStatus::Testing => "Testing…".into(),
            WizardStatus::TestOk { latency_ms, detail } => {
                if detail.is_empty() {
                    format!("OK ({latency_ms} ms)").into()
                } else {
                    format!("OK — {detail} ({latency_ms} ms)").into()
                }
            }
            WizardStatus::TestErr(e) => format!("Error: {e}").into(),
            WizardStatus::Connecting => "Connecting…".into(),
            WizardStatus::ConnectOk => "Connected. Save to keep this connection.".into(),
            WizardStatus::ConnectErr(e) => format!("Error: {e}").into(),
            WizardStatus::Saving => "Saving…".into(),
            WizardStatus::Saved => "Saved.".into(),
            WizardStatus::SaveErr(e) => format!("Error: {e}").into(),
        };
        let show_status = !matches!(self.status, WizardStatus::Idle);
        let is_err = matches!(
            self.status,
            WizardStatus::TestErr(_) | WizardStatus::ConnectErr(_) | WizardStatus::SaveErr(_)
        );

        let tag_chips: Vec<_> = self
            .tags
            .iter()
            .cloned()
            .map(|tag| {
                let label = tag.clone();
                h_flex()
                    .id(SharedString::from(format!("wizard-tag-{tag}")))
                    .cursor_pointer()
                    .child(Tag::secondary().with_size(Size::Small).child(tag))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |panel, _, _, cx| {
                            remove_wizard_tag(&mut panel.tags, &label);
                            cx.notify();
                        }),
                    )
            })
            .collect();

        v_flex()
            .size_full()
            .items_center()
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .w(px(WIZARD_COLUMN_W))
                    .gap_3()
                    .py_4()
                    .px_4()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(heading),
                            )
                            .when(engine == EngineKind::Postgres, |h| {
                                h.child(
                                    Button::new("wizard-import-url")
                                        .ghost()
                                        .label("Import from URL")
                                        .on_click(cx.listener(|panel, _, window, cx| {
                                            panel.open_import_url_dialog(window, cx);
                                        })),
                                )
                            }),
                    )
                    .child(labeled_field(
                        "Engine",
                        muted,
                        Select::new(&self.engine).disabled(editing).w_full(),
                    ))
                    .when(engine == EngineKind::Postgres, |v| {
                        v.child(
                            h_flex()
                                .gap_2()
                                .child(labeled_field(
                                    "Host",
                                    muted,
                                    Input::new(&self.host).cleanable(true).aria_label("Host"),
                                ))
                                .child(labeled_fixed(
                                    "Port",
                                    muted,
                                    96.0,
                                    Input::new(&self.port).cleanable(true).aria_label("Port"),
                                )),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(labeled_field(
                                    "User",
                                    muted,
                                    Input::new(&self.username)
                                        .content_type(InputContentType::Username)
                                        .cleanable(true)
                                        .aria_label("User"),
                                ))
                                .child(labeled_field(
                                    "Password",
                                    muted,
                                    Input::new(&self.password)
                                        .content_type(InputContentType::Password)
                                        .mask_toggle()
                                        .aria_label("Password"),
                                )),
                        )
                        .child(labeled_field(
                            "Database",
                            muted,
                            Input::new(&self.database)
                                .cleanable(true)
                                .aria_label("Database"),
                        ))
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .child(div().text_sm().child("SSL"))
                                .child(
                                    Switch::new("wizard-ssl")
                                        .with_size(Size::Small)
                                        .checked(self.ssl_enabled)
                                        .on_click(cx.listener(|panel, checked, _, cx| {
                                            panel.ssl_enabled = *checked;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .when(self.ssl_enabled, |v| {
                            v.child(labeled_field(
                                "SSL mode",
                                muted,
                                Select::new(&self.ssl_mode).w_full(),
                            ))
                        })
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .child(div().text_sm().child("SSH tunnel"))
                                .child(
                                    Switch::new("wizard-ssh")
                                        .with_size(Size::Small)
                                        .checked(self.ssh_enabled)
                                        .on_click(cx.listener(|panel, checked, _, cx| {
                                            panel.ssh_enabled = *checked;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .when(self.ssh_enabled, |v| {
                            v.child(
                                h_flex()
                                    .gap_2()
                                    .child(labeled_field(
                                        "SSH host",
                                        muted,
                                        Input::new(&self.ssh_host)
                                            .cleanable(true)
                                            .aria_label("SSH host"),
                                    ))
                                    .child(labeled_fixed(
                                        "SSH port",
                                        muted,
                                        96.0,
                                        Input::new(&self.ssh_port)
                                            .cleanable(true)
                                            .aria_label("SSH port"),
                                    )),
                            )
                            .child(labeled_field(
                                "SSH user",
                                muted,
                                Input::new(&self.ssh_user)
                                    .content_type(InputContentType::Username)
                                    .cleanable(true)
                                    .aria_label("SSH user"),
                            ))
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(div().text_xs().text_color(muted).child("Key path"))
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                Input::new(&self.ssh_key_path)
                                                    .cleanable(true)
                                                    .aria_label("SSH key path")
                                                    .flex_1(),
                                            )
                                            .child(
                                                Button::new("wizard-ssh-browse")
                                                    .label("Browse…")
                                                    .on_click(cx.listener(
                                                        |panel, _, window, cx| {
                                                            panel.browse_ssh_key(window, cx);
                                                        },
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .child("Leave empty to use ssh-agent."),
                                    ),
                            )
                            .child(labeled_field(
                                "Key passphrase",
                                muted,
                                Input::new(&self.ssh_key_passphrase)
                                    .content_type(InputContentType::Password)
                                    .mask_toggle()
                                    .aria_label("SSH key passphrase"),
                            ))
                        })
                    })
                    .when(engine == EngineKind::MongoDB, |v| {
                        v.child(labeled_field(
                            "URI",
                            muted,
                            Input::new(&self.mongo_uri)
                                .content_type(InputContentType::Url)
                                .cleanable(true)
                                .aria_label("Connection URI")
                                .font_family(prefs::code_font_family(cx)),
                        ))
                        .child(labeled_field(
                            "Database (optional)",
                            muted,
                            Input::new(&self.mongo_database)
                                .cleanable(true)
                                .aria_label("Database"),
                        ))
                    })
                    .when(engine == EngineKind::SQLite, |v| {
                        v.child(
                            v_flex()
                                .gap_1()
                                .child(div().text_xs().text_color(muted).child("Database path"))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Input::new(&self.sqlite_path)
                                                .cleanable(true)
                                                .aria_label("Database path")
                                                .flex_1(),
                                        )
                                        .child(
                                            Button::new("wizard-sqlite-browse")
                                                .label("Browse…")
                                                .on_click(cx.listener(|panel, _, window, cx| {
                                                    panel.browse_sqlite(window, cx);
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            Checkbox::new("wizard-sqlite-ro")
                                .label("Read-only")
                                .checked(self.sqlite_read_only)
                                .on_click(cx.listener(|panel, checked, _, cx| {
                                    panel.sqlite_read_only = *checked;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("wizard-test")
                                    .label("Test")
                                    .disabled(busy)
                                    .on_click(
                                        cx.listener(|panel, _, _, cx| panel.test_connection(cx)),
                                    ),
                            )
                            .when(!editing, |h| {
                                h.child(
                                    Button::new("wizard-connect")
                                        .primary()
                                        .label("Connect")
                                        .disabled(busy)
                                        .on_click(cx.listener(|panel, _, window, cx| {
                                            panel.connect(window, cx)
                                        })),
                                )
                            }),
                    )
                    .when(show_status, |v| {
                        v.child(
                            div()
                                .text_sm()
                                .when(is_err, |d| d.text_color(cx.theme().red))
                                .child(status),
                        )
                    })
                    .child(Separator::horizontal())
                    .child(labeled_field(
                        "Name",
                        muted,
                        Input::new(&self.name)
                            .cleanable(true)
                            .aria_label("Connection name"),
                    ))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_xs().text_color(muted).child("Tags"))
                            .child(h_flex().gap_1().flex_wrap().children(tag_chips))
                            .child(
                                Input::new(&self.tag_input)
                                    .cleanable(true)
                                    .aria_label("Add a tag"),
                            ),
                    )
                    .when(cx.has_global::<ProjectRoot>() && !editing, |v| {
                        v.child(destination_row(
                            self.destination,
                            muted,
                            cx,
                            |panel, dest| panel.destination = Some(dest),
                        ))
                    })
                    .child(
                        Button::new("wizard-save")
                            .when(editing, |b| b.primary())
                            .label("Save")
                            .disabled(busy)
                            .on_click(cx.listener(|panel, _, window, cx| panel.save(window, cx))),
                    ),
            )
    }
}
