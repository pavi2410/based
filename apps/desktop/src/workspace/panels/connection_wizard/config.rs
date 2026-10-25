use super::*;

impl ConnectionWizardPanel {
    pub fn editing_id(&self) -> Option<&ConnectionId> {
        self.session_id
            .as_ref()
            .filter(|id| can_edit_saved_connection(id))
    }

    pub(super) fn is_edit(&self) -> bool {
        self.editing_id().is_some()
    }

    pub(super) fn apply_config(
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
            ConnectionConfig::Redis(c) => {
                set_field(&self.redis_url, &c.url, window, cx);
            }
            ConnectionConfig::SQLite(c) => {
                set_field(&self.sqlite_path, &c.path.to_string_lossy(), window, cx);
                self.sqlite_read_only = c.read_only;
                self.sqlite_pragma = c.pragma.clone();
            }
        }
    }

    pub(super) fn current_engine(&self, cx: &App) -> EngineKind {
        match self.engine.read(cx).selected_value().copied() {
            Some("MongoDB") => EngineKind::MongoDB,
            Some("SQLite") => EngineKind::SQLite,
            Some("Redis") => EngineKind::Redis,
            _ => EngineKind::Postgres,
        }
    }

    pub(super) fn apply_ssh(
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

    pub(super) fn current_ssh(&self, cx: &App) -> Option<SshTunnelConfig> {
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

    pub(super) fn current_ai(&self, cx: &App) -> Option<AiSettings> {
        let label = self.ai_select.read(cx).selected_value().copied()?;
        ai_from_label(label)
    }

    pub(super) fn current_ssl_mode(&self, cx: &App) -> SslMode {
        let selected = self
            .ssl_mode
            .read(cx)
            .selected_value()
            .copied()
            .and_then(ssl_mode_from_on_label);
        ssl_mode_from_toggle(self.ssl_enabled, selected)
    }

    pub(super) fn config(&self, cx: &App) -> ConnectionConfig {
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
            EngineKind::Redis => ConnectionConfig::Redis(RedisConfig {
                label: self.name.read(cx).value().to_string(),
                url: self.redis_url.read(cx).value().to_string(),
            }),
            EngineKind::SQLite => ConnectionConfig::SQLite(SqliteConfig {
                label: self.name.read(cx).value().to_string(),
                path: PathBuf::from(self.sqlite_path.read(cx).value().as_ref()),
                read_only: self.sqlite_read_only,
                pragma: self.sqlite_pragma.clone(),
            }),
        }
    }

    pub(super) fn labeled_config(&self, cx: &App) -> ConnectionConfig {
        let config = self.config(cx);
        let label = save_label_from_config(self.name.read(cx).value().as_ref(), &config);
        config.with_label(label)
    }

    pub(super) fn apply_uri(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
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

    pub(super) fn open_import_url_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn add_tag_from_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.tag_input.read(cx).value().to_string();
        if add_wizard_tag(&mut self.tags, &raw) {
            set_field(&self.tag_input, "", window, cx);
        }
    }
}

pub(super) fn nonempty_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

pub(super) fn ssl_on_label(mode: SslMode) -> &'static str {
    match mode {
        SslMode::VerifyCa => "verify-ca",
        SslMode::VerifyFull => "verify-full",
        _ => "require",
    }
}

pub(super) fn ssl_mode_from_on_label(label: &str) -> Option<SslMode> {
    Some(match label {
        "require" => SslMode::Require,
        "verify-ca" => SslMode::VerifyCa,
        "verify-full" => SslMode::VerifyFull,
        _ => return None,
    })
}
