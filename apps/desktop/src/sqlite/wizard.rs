//! sqlite::wizard — ConnectionWizardPanel: form for opening a new SQLite connection.

use std::path::PathBuf;

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{Input, InputState},
    menu::PopupMenu,
    v_flex,
};
use tokio::task::spawn_blocking;

use crate::connection::ConnectionConfig;
use crate::connection::OpenedConnection;
use crate::connection::categorize_connect_error;
use crate::connection::lifecycle::Connectable;
use crate::db;
use crate::sqlite::{SqliteConfig, SqliteConnection};
use crate::widgets::{labeled_field, new_field, set_field};
use crate::workspace::WorkspaceRef;

pub enum WizardStatus {
    Idle,
    Testing,
    TestOk { latency_ms: u64, version: String },
    TestErr(String),
    Connecting,
    ConnectErr(String),
}

pub struct ConnectionWizardPanel {
    focus_handle: FocusHandle,
    label: Entity<InputState>,
    path: Entity<InputState>,
    status: WizardStatus,
    pub(crate) tab_label: SharedString,
}

impl ConnectionWizardPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label: new_field(window, cx, "My SQLite DB", "Connection name"),
            path: new_field(window, cx, "", "/path/to/database.db"),
            status: WizardStatus::Idle,
            tab_label: "New SQLite Connection".into(),
        }
    }

    fn config(&self, cx: &App) -> SqliteConfig {
        SqliteConfig {
            label: self.label.read(cx).value().to_string(),
            path: PathBuf::from(self.path.read(cx).value().as_ref()),
            read_only: false,
            pragma: None,
        }
    }

    fn browse_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path_field = self.path.clone();
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

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Testing;
        let config = self.config(cx);
        let task = SqliteConnection::test(&config, cx);

        cx.spawn(async move |this, cx| {
            let result = task.await;
            cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.status = match result {
                        Ok(report) => WizardStatus::TestOk {
                            latency_ms: report.latency_ms,
                            version: report.server_version.unwrap_or_default(),
                        },
                        Err(e) => WizardStatus::TestErr(
                            categorize_connect_error(&e.to_string()).display_message(),
                        ),
                    };
                    cx.notify();
                })
            })
        })
        .detach();
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.status = WizardStatus::Connecting;
        let config = self.config(cx);
        let task = SqliteConnection::open(config.clone(), cx);

        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |panel, cx| match result {
                    Ok(conn) => {
                        let panel_id = cx.entity().entity_id();
                        if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                            ws.update(cx, |workspace, cx| {
                                workspace.finish_wizard_connect(
                                    ConnectionConfig::SQLite(config),
                                    OpenedConnection::Sqlite(conn),
                                    panel_id,
                                    window,
                                    cx,
                                );
                            });
                        } else {
                            db::close_sqlite_pool(conn.pool);
                            panel.status = WizardStatus::ConnectErr("Workspace is not open".into());
                            cx.notify();
                        }
                    }
                    Err(e) => {
                        panel.status = WizardStatus::ConnectErr(
                            categorize_connect_error(&e.to_string()).display_message(),
                        );
                        cx.notify();
                    }
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for ConnectionWizardPanel {}

impl Focusable for ConnectionWizardPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ConnectionWizardPanel {
    fn panel_name(&self) -> &'static str {
        "SqliteWizard"
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

        let status_text: SharedString = match &self.status {
            WizardStatus::Idle => "".into(),
            WizardStatus::Testing => "Testing…".into(),
            WizardStatus::TestOk {
                latency_ms,
                version,
            } => format!("OK — SQLite {version} ({latency_ms}ms)").into(),
            WizardStatus::TestErr(e) => format!("Error: {e}").into(),
            WizardStatus::Connecting => "Connecting…".into(),
            WizardStatus::ConnectErr(e) => format!("Error: {e}").into(),
        };

        let show_status = !matches!(self.status, WizardStatus::Idle);
        let is_error = matches!(
            self.status,
            WizardStatus::TestErr(_) | WizardStatus::ConnectErr(_)
        );

        v_flex()
            .size_full()
            .gap_2()
            .p_3()
            .child(labeled_field(
                "Label",
                muted,
                Input::new(&self.label).cleanable(true).aria_label("Label"),
            ))
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_xs().text_color(muted).child("Database path"))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Input::new(&self.path)
                                    .cleanable(true)
                                    .aria_label("Database path")
                                    .flex_1(),
                            )
                            .child(Button::new("sqlite-browse").label("Browse…").on_click(
                                cx.listener(|panel, _, window, cx| {
                                    panel.browse_path(window, cx);
                                }),
                            )),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("sqlite-test")
                            .label("Test")
                            .on_click(cx.listener(|panel, _, _, cx| panel.test_connection(cx))),
                    )
                    .child(
                        Button::new("sqlite-connect")
                            .primary()
                            .label("Connect")
                            .on_click(
                                cx.listener(|panel, _, window, cx| panel.connect(window, cx)),
                            ),
                    ),
            )
            .when(show_status, |v| {
                v.child(
                    div()
                        .text_sm()
                        .when(is_error, |d| d.text_color(cx.theme().red))
                        .child(status_text),
                )
            })
    }
}
