// postgres::wizard — connect / test with optional `postgresql://` URI paste.

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, IndexPath,
    button::{Button, ButtonVariants},
    dock::{Panel, PanelEvent},
    h_flex,
    input::{Input, InputContentType, InputState},
    menu::PopupMenu,
    select::{Select, SelectState},
    v_flex,
};

use crate::app::prefs;
use crate::connection::ConnectionConfig;
use crate::connection::categorize_connect_error;
use crate::connection::lifecycle::Connectable;
use crate::postgres::{PgConnection, PostgresConfig, SslMode};
use crate::workspace::WorkspaceRef;

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
    label: Entity<InputState>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    database: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    ssl_mode: Entity<SelectState<Vec<&'static str>>>,
    uri: Entity<InputState>,
    status: WizardStatus,
    pub(crate) tab_label: SharedString,
}

impl ConnectionWizardPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label: new_field(window, cx, "PostgreSQL", "Connection name"),
            host: new_field(window, cx, "localhost", "Host"),
            port: new_field(window, cx, "5432", "Port"),
            database: new_field(window, cx, "postgres", "Database"),
            username: new_field(window, cx, "postgres", "Username"),
            password: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Password")
                    .masked(true)
            }),
            ssl_mode: cx.new(|cx| {
                SelectState::new(
                    SSL_MODE_LABELS.to_vec(),
                    Some(IndexPath::new(ssl_mode_index(SslMode::Prefer))),
                    window,
                    cx,
                )
            }),
            uri: new_field(window, cx, "", "postgresql://user:pass@host:5432/db"),
            status: WizardStatus::Idle,
            tab_label: "New PostgreSQL connection".into(),
        }
    }

    fn config(&self, cx: &App) -> PostgresConfig {
        let port = self.port.read(cx).value().parse().unwrap_or(5432u16);
        PostgresConfig {
            label: self.label.read(cx).value().to_string(),
            host: self.host.read(cx).value().to_string(),
            port,
            database: self.database.read(cx).value().to_string(),
            username: self.username.read(cx).value().to_string(),
            password: self.password.read(cx).value().to_string(),
            ssl_mode: self.current_ssl_mode(cx),
        }
    }

    fn apply_uri(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let uri = self.uri.read(cx).value().to_string();
        let Some(cfg) = parse_postgres_uri(&uri) else {
            self.status = WizardStatus::TestErr("Could not parse URI".into());
            return;
        };
        set_field(&self.label, &cfg.label, window, cx);
        set_field(&self.host, &cfg.host, window, cx);
        set_field(&self.port, &cfg.port.to_string(), window, cx);
        set_field(&self.database, &cfg.database, window, cx);
        set_field(&self.username, &cfg.username, window, cx);
        set_field(&self.password, &cfg.password, window, cx);
        let ssl_label = ssl_mode_label(cfg.ssl_mode);
        self.ssl_mode.update(cx, |select, cx| {
            select.set_selected_value(&ssl_label, window, cx);
        });
        self.status = WizardStatus::Idle;
    }

    fn current_ssl_mode(&self, cx: &App) -> SslMode {
        self.ssl_mode
            .read(cx)
            .selected_value()
            .copied()
            .and_then(ssl_mode_from_label)
            .unwrap_or(SslMode::Prefer)
    }

    fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Testing;
        let config = self.config(cx);
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
        let config = self.config(cx);
        let task = PgConnection::open(config.clone(), cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            cx.update(|cx| {
                this.update(cx, |panel, cx| match result {
                    Ok(conn) => {
                        if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                            ws.update(cx, |workspace, cx| {
                                workspace.persist_connection_config(
                                    &ConnectionConfig::Postgres(config),
                                    cx,
                                );
                            });
                        }
                        cx.emit(WizardEvent::Connected(conn));
                    }
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

    crate::based_panel_tab_chrome!();
}

impl Render for ConnectionWizardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .items_center()
                    .child(
                        Input::new(&self.uri)
                            .content_type(InputContentType::Url)
                            .cleanable(true)
                            .aria_label("Connection URI")
                            .font_family(prefs::code_font_family(cx))
                            .flex_1(),
                    )
                    .child(
                        Button::new("pg-parse-uri")
                            .label("Apply URI")
                            .on_click(cx.listener(|panel, _, window, cx| {
                                panel.apply_uri(window, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(div().text_xs().text_color(muted).child("Manual fields"))
            .child(
                h_flex()
                    .gap_2()
                    .child(labeled_field(
                        "Label",
                        muted,
                        Input::new(&self.label).cleanable(true).aria_label("Label"),
                    ))
                    .child(labeled_field(
                        "Host",
                        muted,
                        Input::new(&self.host).cleanable(true).aria_label("Host"),
                    )),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(labeled_field(
                        "Port",
                        muted,
                        Input::new(&self.port).cleanable(true).aria_label("Port"),
                    ))
                    .child(labeled_field(
                        "Database",
                        muted,
                        Input::new(&self.database)
                            .cleanable(true)
                            .aria_label("Database"),
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
                "SSL mode",
                muted,
                Select::new(&self.ssl_mode).w_full(),
            ))
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

fn new_field(
    window: &mut Window,
    cx: &mut Context<ConnectionWizardPanel>,
    default: &str,
    placeholder: &str,
) -> Entity<InputState> {
    let default = default.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(default)
    })
}

fn set_field(input: &Entity<InputState>, value: &str, window: &mut Window, cx: &mut App) {
    input.update(cx, |state, cx| {
        state.set_value(value, window, cx);
    });
}

fn labeled_field(title: &str, muted: Hsla, input: impl IntoElement) -> impl IntoElement {
    v_flex()
        .flex_1()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(muted)
                .child(SharedString::from(title.to_string())),
        )
        .child(div().w_full().child(input))
}

const SSL_MODE_LABELS: &[&str] = &["disable", "prefer", "require", "verify-ca", "verify-full"];

fn ssl_mode_label(mode: SslMode) -> &'static str {
    match mode {
        SslMode::Disable => "disable",
        SslMode::Prefer => "prefer",
        SslMode::Require => "require",
        SslMode::VerifyCa => "verify-ca",
        SslMode::VerifyFull => "verify-full",
    }
}

fn ssl_mode_from_label(label: &str) -> Option<SslMode> {
    Some(match label {
        "disable" => SslMode::Disable,
        "prefer" => SslMode::Prefer,
        "require" => SslMode::Require,
        "verify-ca" => SslMode::VerifyCa,
        "verify-full" => SslMode::VerifyFull,
        _ => return None,
    })
}

fn ssl_mode_index(mode: SslMode) -> usize {
    SSL_MODE_LABELS
        .iter()
        .position(|label| *label == ssl_mode_label(mode))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::{
        SSL_MODE_LABELS, parse_postgres_uri, ssl_mode_from_label, ssl_mode_index, ssl_mode_label,
    };
    use crate::postgres::SslMode;

    #[test]
    fn parse_full_uri() {
        let cfg = parse_postgres_uri(
            "postgresql://alice:s3cret@db.example:6543/analytics?sslmode=require",
        )
        .expect("uri should parse");
        assert_eq!(cfg.username, "alice");
        assert_eq!(cfg.password, "s3cret");
        assert_eq!(cfg.host, "db.example");
        assert_eq!(cfg.port, 6543);
        assert_eq!(cfg.database, "analytics");
        assert_eq!(cfg.label, "analytics");
        assert!(matches!(cfg.ssl_mode, SslMode::Require));
    }

    #[test]
    fn parse_postgres_scheme_and_defaults() {
        let cfg = parse_postgres_uri("postgres://localhost/").expect("uri should parse");
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 5432);
        assert_eq!(cfg.database, "postgres");
        assert_eq!(cfg.username, "postgres");
        assert!(cfg.password.is_empty());
        assert!(matches!(cfg.ssl_mode, SslMode::Prefer));
    }

    #[test]
    fn parse_url_encoded_password() {
        let cfg = parse_postgres_uri("postgresql://user:p%40ss@localhost/db").unwrap();
        assert_eq!(cfg.password, "p@ss");
    }

    #[test]
    fn parse_rejects_non_postgres_uri() {
        assert!(parse_postgres_uri("mysql://localhost/db").is_none());
        assert!(parse_postgres_uri("not a uri").is_none());
    }

    #[test]
    fn ssl_mode_labels_round_trip() {
        assert_eq!(SSL_MODE_LABELS.len(), 5);
        for (ix, label) in SSL_MODE_LABELS.iter().enumerate() {
            let mode = ssl_mode_from_label(label).expect("known sslmode");
            assert_eq!(ssl_mode_label(mode), *label);
            assert_eq!(ssl_mode_index(mode), ix);
        }
        assert!(ssl_mode_from_label("nope").is_none());
    }
}
