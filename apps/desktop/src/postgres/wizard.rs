// postgres::wizard — connect / test with optional `postgresql://` URI paste.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Theme,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    menu::PopupMenu,
    v_flex,
};

use crate::connection::categorize_connect_error;
use crate::connection::lifecycle::Connectable;
use crate::postgres::{PgConnection, PostgresConfig, SslMode};

pub enum WizardStatus {
    Idle,
    Testing,
    TestOk { latency_ms: u64, version: String },
    TestErr(String),
    Connecting,
    ConnectErr(String),
}

pub enum WizardEvent {
    Connected(PgConnection),
}

pub struct ConnectionWizardPanel {
    focus_handle: FocusHandle,
    label: String,
    host: String,
    port: String,
    database: String,
    username: String,
    password: String,
    ssl_mode: SslMode,
    uri: String,
    status: WizardStatus,
    pub(crate) tab_label: SharedString,
}

impl ConnectionWizardPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label: String::from("PostgreSQL"),
            host: String::from("localhost"),
            port: String::from("5432"),
            database: String::from("postgres"),
            username: String::from("postgres"),
            password: String::new(),
            ssl_mode: SslMode::Prefer,
            uri: String::new(),
            status: WizardStatus::Idle,
            tab_label: "New PostgreSQL connection".into(),
        }
    }

    fn config(&self) -> PostgresConfig {
        let port = self.port.parse().unwrap_or(5432u16);
        PostgresConfig {
            label: self.label.clone(),
            host: self.host.clone(),
            port,
            database: self.database.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            ssl_mode: self.ssl_mode,
        }
    }

    fn apply_uri(&mut self) {
        let Some(cfg) = parse_postgres_uri(&self.uri) else {
            self.status = WizardStatus::TestErr("Could not parse URI".into());
            return;
        };
        self.label = cfg.label.clone();
        self.host = cfg.host;
        self.port = cfg.port.to_string();
        self.database = cfg.database;
        self.username = cfg.username;
        self.password = cfg.password;
        self.ssl_mode = cfg.ssl_mode;
        self.status = WizardStatus::Idle;
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Testing;
        let config = self.config();
        let task = PgConnection::test(&config, cx);
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

    fn connect(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Connecting;
        let config = self.config();
        let task = PgConnection::open(config, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            cx.update(|cx| {
                this.update(cx, |panel, cx| match result {
                    Ok(conn) => cx.emit(WizardEvent::Connected(conn)),
                    Err(e) => {
                        panel.status = WizardStatus::ConnectErr(
                            categorize_connect_error(&e.to_string()).display_message(),
                        );
                        cx.notify();
                    }
                })
            })
        })
        .detach();
    }
}

/// Minimal `postgresql://user:pass@host:port/db?sslmode=prefer` parser.
fn parse_postgres_uri(input: &str) -> Option<PostgresConfig> {
    let s = input.trim();
    let rest = s
        .strip_prefix("postgresql://")
        .or_else(|| s.strip_prefix("postgres://"))?;

    let (credentials, after_at) = match rest.split_once('@') {
        Some((c, h)) => (c, h),
        None => ("", rest),
    };

    let (username, password) = if credentials.is_empty() {
        ("postgres".to_string(), String::new())
    } else if let Some((u, p)) = credentials.split_once(':') {
        (url_decode(u), url_decode(p))
    } else {
        (url_decode(credentials), String::new())
    };

    let (host_part, path_part) = after_at
        .split_once('/')
        .map(|(a, b)| (a, Some(b)))
        .unwrap_or((after_at, None));

    let (host, port) = if let Some((h, p)) = host_part.split_once(':') {
        (h.to_string(), p.parse().unwrap_or(5432u16))
    } else {
        (host_part.to_string(), 5432u16)
    };

    let (database, ssl_mode) = path_part
        .map(parse_path_and_query)
        .unwrap_or(("postgres".to_string(), SslMode::Prefer));

    Some(PostgresConfig {
        label: database.clone(),
        host,
        port,
        database,
        username,
        password,
        ssl_mode,
    })
}

fn parse_path_and_query(path_query: &str) -> (String, SslMode) {
    let (path, query) = path_query
        .split_once('?')
        .map(|(p, q)| (p, Some(q)))
        .unwrap_or((path_query, None));
    let db = if path.is_empty() {
        "postgres".to_string()
    } else {
        path.to_string()
    };
    let ssl = query
        .and_then(|q| {
            q.split('&').find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                if k == "sslmode" {
                    Some(ssl_mode_from_str(v))
                } else {
                    None
                }
            })
        })
        .unwrap_or(SslMode::Prefer);
    (db, ssl)
}

fn ssl_mode_from_str(s: &str) -> SslMode {
    match s.to_ascii_lowercase().as_str() {
        "disable" | "off" => SslMode::Disable,
        "require" => SslMode::Require,
        "verify-ca" => SslMode::VerifyCa,
        "verify-full" => SslMode::VerifyFull,
        _ => SslMode::Prefer,
    }
}

fn url_decode(s: &str) -> String {
    // Only handle %XX for common cases; otherwise return as-is.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let a = chars.next();
            let b = chars.next();
            if let (Some(a), Some(b)) = (a, b)
                && let Ok(byte) = u8::from_str_radix(&format!("{a}{b}"), 16)
            {
                out.push(byte as char);
                continue;
            }
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out
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
        "PgWizard"
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

    crate::based_panel_tab_chrome!();

    fn title(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.tab_label.clone()
    }
}

impl Render for ConnectionWizardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;

        let status: SharedString = match &self.status {
            WizardStatus::Idle => "".into(),
            WizardStatus::Testing => "Testing…".into(),
            WizardStatus::TestOk {
                latency_ms,
                version,
            } => format!("OK — {version} ({latency_ms} ms)").into(),
            WizardStatus::TestErr(e) => format!("Error: {e}").into(),
            WizardStatus::Connecting => "Connecting…".into(),
            WizardStatus::ConnectErr(e) => format!("Error: {e}").into(),
        };

        let show_status = !matches!(self.status, WizardStatus::Idle);
        let is_err = matches!(
            self.status,
            WizardStatus::TestErr(_) | WizardStatus::ConnectErr(_)
        );
        let theme = cx.theme();

        v_flex()
            .size_full()
            .gap_2()
            .p_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Paste connection URI (optional)"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .p_2()
                            .border_1()
                            .border_color(border)
                            .font_family("monospace")
                            .text_xs()
                            .child(self.uri.clone()),
                    )
                    .child(
                        Button::new("pg-parse-uri")
                            .label("Apply URI")
                            .on_click(cx.listener(|panel, _, _, cx| {
                                panel.apply_uri();
                                cx.notify();
                            })),
                    ),
            )
            .child(div().text_xs().text_color(muted).child("Manual fields"))
            .child(
                h_flex()
                    .gap_2()
                    .child(field_line("Label", &self.label, theme))
                    .child(field_line("Host", &self.host, theme)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(field_line("Port", &self.port, theme))
                    .child(field_line("Database", &self.database, theme)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(field_line("User", &self.username, theme))
                    .child(field_line(
                        "Password",
                        if self.password.is_empty() {
                            "(empty)"
                        } else {
                            "••••••"
                        },
                        theme,
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("SSL mode: use URI or defaults (Prefer)"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("pg-test")
                            .label("Test")
                            .on_click(cx.listener(|panel, _, _, cx| panel.test_connection(cx))),
                    )
                    .child(
                        Button::new("pg-connect")
                            .primary()
                            .label("Connect")
                            .on_click(cx.listener(|panel, _, _, cx| panel.connect(cx))),
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
    }
}

fn field_line(title: &str, value: &str, theme: &Theme) -> impl IntoElement {
    let border = theme.border;
    v_flex()
        .flex_1()
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
                .text_sm()
                .child(value.to_string()),
        )
}
