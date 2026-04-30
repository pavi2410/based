// sqlite::wizard — ConnectionWizardPanel: form for opening a new SQLite connection.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme,
    button::Button,
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};

use crate::connection::lifecycle::Connectable;
use crate::sqlite::{SqliteConfig, SqliteConnection};

pub enum WizardStatus {
    Idle,
    Testing,
    TestOk { latency_ms: u64, version: String },
    TestErr(String),
    Connecting,
    ConnectErr(String),
}

pub enum WizardEvent {
    Connected(SqliteConnection),
}

pub struct ConnectionWizardPanel {
    focus_handle: FocusHandle,
    label: String,
    path: String,
    wal: bool,
    status: WizardStatus,
}

impl ConnectionWizardPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label: String::from("My SQLite DB"),
            path: String::new(),
            wal: false,
            status: WizardStatus::Idle,
        }
    }

    fn config(&self) -> SqliteConfig {
        SqliteConfig {
            label: self.label.clone(),
            path: std::path::PathBuf::from(&self.path),
            wal: self.wal,
        }
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Testing;
        let config = self.config();
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
                        Err(e) => WizardStatus::TestErr(e.to_string()),
                    };
                    cx.notify();
                })
            })
        })
        .detach();
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Connecting;
        let config = self.config();
        let task = SqliteConnection::open(config, cx);

        cx.spawn(async move |this, cx| {
            let result = task.await;
            cx.update(|cx| {
                this.update(cx, |panel, cx| match result {
                    Ok(conn) => {
                        cx.emit(WizardEvent::Connected(conn));
                    }
                    Err(e) => {
                        panel.status = WizardStatus::ConnectErr(e.to_string());
                        cx.notify();
                    }
                })
            })
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for ConnectionWizardPanel {}
impl EventEmitter<WizardEvent> for ConnectionWizardPanel {}

impl Focusable for ConnectionWizardPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ConnectionWizardPanel {
    fn panel_name(&self) -> &'static str {
        "SqliteWizard"
    }

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "New SQLite Connection"
    }
}

impl Render for ConnectionWizardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;

        let status_text: SharedString = match &self.status {
            WizardStatus::Idle => "".into(),
            WizardStatus::Testing => "Testing…".into(),
            WizardStatus::TestOk { latency_ms, version } => {
                format!("OK — SQLite {version} ({latency_ms}ms)").into()
            }
            WizardStatus::TestErr(e) => format!("Error: {e}").into(),
            WizardStatus::Connecting => "Connecting…".into(),
            WizardStatus::ConnectErr(e) => format!("Error: {e}").into(),
        };

        let is_error = matches!(
            self.status,
            WizardStatus::TestErr(_) | WizardStatus::ConnectErr(_)
        );

        let label_val: SharedString = self.label.clone().into();
        let path_val: SharedString = self.path.clone().into();
        let wal_val: SharedString = if self.wal { "WAL: ON" } else { "WAL: OFF" }.into();

        v_flex()
            .w_full()
            .h_full()
            .p(px(16.0))
            .gap(px(12.0))
            .child(
                v_flex()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Label"),
                    )
                    .child(
                        div()
                            .id("wizard-label")
                            .border_1()
                            .border_color(border)
                            .rounded(px(4.0))
                            .px(px(8.0))
                            .py(px(4.0))
                            .text_sm()
                            .child(label_val),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Database Path"),
                    )
                    .child(
                        div()
                            .id("wizard-path")
                            .border_1()
                            .border_color(border)
                            .rounded(px(4.0))
                            .px(px(8.0))
                            .py(px(4.0))
                            .text_sm()
                            .child(path_val),
                    ),
            )
            .child(
                div()
                    .id("wizard-wal")
                    .border_1()
                    .border_color(border)
                    .rounded(px(4.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .text_sm()
                    .child(wal_val),
            )
            .child(
                h_flex()
                    .gap(px(8.0))
                    .child(
                        Button::new("test")
                            .label("Test Connection")
                            .on_click(
                                cx.listener(|panel, _, _window, cx| panel.test_connection(cx)),
                            ),
                    )
                    .child(
                        Button::new("connect")
                            .label("Connect")
                            .on_click(cx.listener(|panel, _, _window, cx| panel.connect(cx))),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .when(is_error, |d| d.text_color(rgb(0xff5555)))
                    .when(!is_error, |d| d.text_color(muted))
                    .child(status_text),
            )
    }
}
