//! First step of "New connection": paste a connection string or choose a database.

use std::fs;

use super::*;

use crate::widgets::engine_icon;

const CHOICES: [(EngineKind, &str, &str); 4] = [
    (
        EngineKind::Postgres,
        "PostgreSQL",
        "Host, port, SSL, SSH tunnel",
    ),
    (
        EngineKind::MongoDB,
        "MongoDB",
        "Connection string, replica sets, Atlas",
    ),
    (EngineKind::SQLite, "SQLite", "Open a local database file"),
    (EngineKind::Redis, "Redis", "URL, TLS, ACL user"),
];

/// Which engine a pasted string belongs to, from its scheme.
pub(super) fn engine_for_uri(input: &str) -> Option<EngineKind> {
    let scheme = input.trim().split("://").next()?.to_ascii_lowercase();
    match scheme.split('+').next()? {
        "postgres" | "postgresql" => Some(EngineKind::Postgres),
        "mongodb" => Some(EngineKind::MongoDB),
        "redis" | "rediss" => Some(EngineKind::Redis),
        _ => None,
    }
}

fn is_sqlite_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".db", ".sqlite", ".sqlite3"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

impl ConnectionWizardPanel {
    fn choose_engine(&mut self, label: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.engine
            .update(cx, |s, cx| s.set_selected_value(&label, window, cx));
        self.status = WizardStatus::Idle;
        self.stage = Stage::Form;
        cx.notify();
    }

    /// Fill the form from the pasted text and move on; reports why when it can't.
    pub(super) fn detect_pasted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.paste.read(cx).value().trim().to_string();
        if text.is_empty() {
            self.paste_error = Some("Paste a connection string first.".into());
            cx.notify();
            return;
        }
        match engine_for_uri(&text) {
            Some(EngineKind::Postgres) => {
                set_field(&self.uri, &text, window, cx);
                if !self.apply_uri(window, cx) {
                    self.paste_error = Some("That PostgreSQL address could not be parsed.".into());
                    self.status = WizardStatus::Idle;
                    cx.notify();
                    return;
                }
                self.choose_engine("PostgreSQL", window, cx);
            }
            Some(EngineKind::MongoDB) => {
                set_field(&self.mongo_uri, &text, window, cx);
                self.choose_engine("MongoDB", window, cx);
            }
            Some(EngineKind::Redis) => {
                set_field(&self.redis_url, &text, window, cx);
                self.choose_engine("Redis", window, cx);
            }
            _ if is_sqlite_file(&text) => self.use_sqlite_file(PathBuf::from(&text), window, cx),
            _ => {
                self.paste_error = Some(
                    "Not recognised. Try postgres://, mongodb://, redis:// or a .db file path."
                        .into(),
                );
                cx.notify();
            }
        }
    }

    fn use_sqlite_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        set_field(&self.sqlite_path, &path.to_string_lossy(), window, cx);
        if let Some(stem) = path.file_stem() {
            set_field(&self.name, &stem.to_string_lossy(), window, cx);
        }
        self.choose_engine("SQLite", window, cx);
    }

    /// SQLite files sitting at the top of the open project.
    fn detected_sqlite_files(cx: &App) -> Vec<PathBuf> {
        let Some(root) = cx.try_global::<ProjectRoot>().map(|r| r.0.clone()) else {
            return Vec::new();
        };
        let Ok(dir) = fs::read_dir(root) else {
            return Vec::new();
        };
        let mut found: Vec<PathBuf> = dir
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(is_sqlite_file)
            })
            .collect();
        found.sort();
        found.truncate(4);
        found
    }

    pub(super) fn render_picker(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (border, muted, fg, hover) = (
            theme.border,
            theme.muted_foreground,
            theme.foreground,
            theme.muted.opacity(0.45),
        );
        let detected = Self::detected_sqlite_files(cx);

        let cards = CHOICES.map(|(engine, label, hint)| {
            v_flex()
                .id(SharedString::from(format!("pick-{label}")))
                .w_full()
                .gap_2()
                .p_3()
                .rounded(px(10.0))
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .hover(move |s| s.bg(hover))
                .child(engine_icon(engine))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(fg)
                        .child(label),
                )
                .child(div().text_xs().text_color(muted).child(hint))
                .on_click(cx.listener(move |panel, _, window, cx| {
                    panel.choose_engine(label, window, cx);
                }))
        });
        let [pg, mongo, sqlite, redis] = cards;

        v_flex()
            .size_full()
            .items_center()
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .w(px(WIZARD_COLUMN_W))
                    .gap_4()
                    .py_6()
                    .px_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Add a connection"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(muted)
                                    .child("Choose a database, or paste a connection string."),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div().flex_1().child(
                                    Input::new(&self.paste)
                                        .cleanable(true)
                                        .aria_label("Connection string"),
                                ),
                            )
                            .child(
                                Button::new("pick-detect")
                                    .primary()
                                    .label("Continue")
                                    .on_click(cx.listener(|panel, _, window, cx| {
                                        panel.detect_pasted(window, cx);
                                    })),
                            ),
                    )
                    .when_some(self.paste_error.clone(), |v, msg| {
                        v.child(div().text_xs().text_color(cx.theme().red).child(msg))
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().h(px(1.0)).bg(border))
                            .child(div().text_xs().text_color(muted).child("OR CHOOSE"))
                            .child(div().flex_1().h(px(1.0)).bg(border)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(h_flex().gap_2().child(pg).child(mongo))
                            .child(h_flex().gap_2().child(sqlite).child(redis)),
                    )
                    .when(!detected.is_empty(), |v| {
                        v.child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child("FOUND IN THIS PROJECT"),
                                )
                                .children(detected.into_iter().enumerate().map(|(i, path)| {
                                    let name = path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default();
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .px_3()
                                        .py_2()
                                        .rounded(px(8.0))
                                        .border_1()
                                        .border_color(border)
                                        .child(engine_icon(EngineKind::SQLite))
                                        .child(div().flex_1().text_sm().child(name))
                                        .child(
                                            Button::new(("pick-use", i))
                                                .small()
                                                .outline()
                                                .label("Use")
                                                .on_click(cx.listener(
                                                    move |panel, _, window, cx| {
                                                        panel.use_sqlite_file(
                                                            path.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                )),
                                        )
                                })),
                        )
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{engine_for_uri, is_sqlite_file};
    use crate::connection::EngineKind;

    #[test]
    fn detects_engine_from_scheme() {
        assert_eq!(
            engine_for_uri("postgres://u:p@h:5432/db"),
            Some(EngineKind::Postgres)
        );
        assert_eq!(
            engine_for_uri("postgresql://h/db"),
            Some(EngineKind::Postgres)
        );
        assert_eq!(
            engine_for_uri("mongodb+srv://c.example.net/x"),
            Some(EngineKind::MongoDB)
        );
        assert_eq!(engine_for_uri("rediss://h:6379"), Some(EngineKind::Redis));
        assert_eq!(engine_for_uri("/tmp/app.db"), None);
    }

    #[test]
    fn recognises_sqlite_files() {
        assert!(is_sqlite_file("dev.sqlite3"));
        assert!(is_sqlite_file("APP.DB"));
        assert!(!is_sqlite_file("notes.txt"));
    }
}
