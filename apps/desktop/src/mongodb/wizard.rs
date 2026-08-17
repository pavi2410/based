//! mongodb::wizard — connect / test from URI string.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{Input, InputContentType, InputState},
    menu::PopupMenu,
    v_flex,
};

use crate::app::prefs;
use crate::connection::ConnectionConfig;
use crate::connection::OpenedConnection;
use crate::connection::categorize_connect_error;
use crate::connection::lifecycle::Connectable;
use crate::mongodb::{MongoConfig, MongoConnection};
use crate::widgets::{labeled_field, new_field};
use crate::workspace::WorkspaceRef;

pub enum WizardStatus {
    Idle,
    Testing,
    TestOk { latency_ms: u64, detail: String },
    TestErr(String),
    Connecting,
    ConnectErr(String),
}

pub struct ConnectionWizardPanel {
    focus_handle: FocusHandle,
    label: Entity<InputState>,
    uri: Entity<InputState>,
    database: Entity<InputState>,
    auth_source: Entity<InputState>,
    status: WizardStatus,
    pub(crate) tab_label: SharedString,
}

impl ConnectionWizardPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label: new_field(window, cx, "MongoDB", "Connection name"),
            uri: new_field(
                window,
                cx,
                "mongodb://127.0.0.1:27017",
                "mongodb://host:27017",
            ),
            database: new_field(window, cx, "", "Database override (optional)"),
            auth_source: new_field(window, cx, "", "authSource (optional)"),
            status: WizardStatus::Idle,
            tab_label: "New MongoDB connection".into(),
        }
    }

    fn config(&self, cx: &App) -> MongoConfig {
        let database = self.database.read(cx).value().to_string();
        let auth_source = self.auth_source.read(cx).value().to_string();
        MongoConfig {
            label: self.label.read(cx).value().to_string(),
            uri: self.uri.read(cx).value().to_string(),
            database: if database.trim().is_empty() {
                None
            } else {
                Some(database)
            },
            auth_source: if auth_source.trim().is_empty() {
                None
            } else {
                Some(auth_source)
            },
        }
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Testing;
        let config = self.config(cx);
        let task = MongoConnection::test(&config, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.status = match result {
                        Ok(r) => WizardStatus::TestOk {
                            latency_ms: r.latency_ms,
                            detail: r.message.unwrap_or_default(),
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
        let task = MongoConnection::open(config.clone(), cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |panel, cx| match result {
                    Ok(conn) => {
                        let panel_id = cx.entity().entity_id();
                        if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                            ws.update(cx, |workspace, cx| {
                                workspace.finish_wizard_connect(
                                    ConnectionConfig::MongoDB(config),
                                    OpenedConnection::MongoDB(conn),
                                    panel_id,
                                    window,
                                    cx,
                                );
                            });
                        } else {
                            drop(conn);
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
        "MongoWizard"
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

        let status: SharedString = match &self.status {
            WizardStatus::Idle => "".into(),
            WizardStatus::Testing => "Testing…".into(),
            WizardStatus::TestOk { latency_ms, detail } => {
                format!("OK ({latency_ms} ms) — {detail}").into()
            }
            WizardStatus::TestErr(e) => format!("Error: {e}").into(),
            WizardStatus::Connecting => "Connecting…".into(),
            WizardStatus::ConnectErr(e) => format!("Error: {e}").into(),
        };

        let show_status = !matches!(self.status, WizardStatus::Idle);
        let is_err = matches!(
            self.status,
            WizardStatus::TestErr(_) | WizardStatus::ConnectErr(_)
        );

        v_flex()
            .size_full()
            .gap_2()
            .p_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Connection"),
            )
            .child(labeled_field(
                "Label",
                muted,
                Input::new(&self.label).cleanable(true).aria_label("Label"),
            ))
            .child(labeled_field(
                "URI",
                muted,
                Input::new(&self.uri)
                    .content_type(InputContentType::Url)
                    .cleanable(true)
                    .aria_label("Connection URI")
                    .font_family(prefs::code_font_family(cx)),
            ))
            .child(labeled_field(
                "Database override (optional)",
                muted,
                Input::new(&self.database)
                    .cleanable(true)
                    .aria_label("Database override"),
            ))
            .child(labeled_field(
                "authSource (optional)",
                muted,
                Input::new(&self.auth_source)
                    .cleanable(true)
                    .aria_label("authSource"),
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("URI may include credentials; database override wins over URI path."),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("mongo-test")
                            .label("Test")
                            .on_click(cx.listener(|p, _, _, cx| p.test_connection(cx))),
                    )
                    .child(
                        Button::new("mongo-connect")
                            .primary()
                            .label("Connect")
                            .on_click(cx.listener(|p, _, window, cx| p.connect(window, cx))),
                    ),
            )
            .when(show_status, |v| {
                v.child(
                    div()
                        .text_sm()
                        .when(is_err, |d| d.text_color(cx.theme().red))
                        .child(status),
                )
            })
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("URI example: mongodb://localhost:27017/mydb?authSource=admin"),
            )
    }
}
