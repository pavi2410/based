// mongodb::wizard — connect / test from URI string.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Theme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    v_flex,
};

use crate::connection::lifecycle::Connectable;
use crate::mongodb::{MongoConfig, MongoConnection};

pub enum WizardStatus {
    Idle,
    Testing,
    TestOk { latency_ms: u64, detail: String },
    TestErr(String),
    Connecting,
    ConnectErr(String),
}

pub enum WizardEvent {
    Connected(MongoConnection),
}

pub struct ConnectionWizardPanel {
    focus_handle: FocusHandle,
    label: String,
    uri: String,
    database: String,
    auth_source: String,
    status: WizardStatus,
}

impl ConnectionWizardPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label: String::from("MongoDB"),
            uri: String::from("mongodb://127.0.0.1:27017"),
            database: String::new(),
            auth_source: String::new(),
            status: WizardStatus::Idle,
        }
    }

    fn config(&self) -> MongoConfig {
        MongoConfig {
            label: self.label.clone(),
            uri: self.uri.clone(),
            database: if self.database.trim().is_empty() {
                None
            } else {
                Some(self.database.clone())
            },
            auth_source: if self.auth_source.trim().is_empty() {
                None
            } else {
                Some(self.auth_source.clone())
            },
        }
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Testing;
        let config = self.config();
        let task = MongoConnection::test(&config, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| {
                    panel.status = match result {
                        Ok(r) => WizardStatus::TestOk {
                            latency_ms: r.latency_ms,
                            detail: r.message.unwrap_or_default(),
                        },
                        Err(e) => WizardStatus::TestErr(e.to_string()),
                    };
                    cx.notify();
                })
            });
        })
        .detach();
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Connecting;
        let config = self.config();
        let task = MongoConnection::open(config, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|cx| {
                this.update(cx, |panel, cx| match result {
                    Ok(conn) => {
                        cx.emit(WizardEvent::Connected(conn));
                    }
                    Err(e) => {
                        panel.status = WizardStatus::ConnectErr(e.to_string());
                        cx.notify();
                    }
                })
            });
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

    fn closable(&self, _: &App) -> bool {
        true
    }

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "New MongoDB connection"
    }
}

impl Render for ConnectionWizardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let theme = cx.theme();
        let show_status = !matches!(self.status, WizardStatus::Idle);
        let is_err = matches!(
            self.status,
            WizardStatus::TestErr(_) | WizardStatus::ConnectErr(_)
        );

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
            .child(field_line("Label", &self.label, theme))
            .child(field_line("URI", &self.uri, theme))
            .child(field_line(
                "Database override (optional)",
                &self.database,
                theme,
            ))
            .child(field_line(
                "authSource (optional)",
                &self.auth_source,
                theme,
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
                            .on_click(cx.listener(|p, _, _, cx| p.connect(cx))),
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
                    .p_2()
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(muted)
                    .child("URI examples: mongodb://localhost:27017/mydb?authSource=admin"),
            )
    }
}

fn field_line(title: &str, value: &str, theme: &Theme) -> impl IntoElement {
    let border = theme.border;
    v_flex()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(title.to_string())),
        )
        .child(
            div()
                .p_2()
                .border_1()
                .border_color(border)
                .font_family("monospace")
                .text_sm()
                .child(value.to_string()),
        )
}
