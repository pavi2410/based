use super::*;

impl ConnectionWizardPanel {
    pub(super) fn test_connection(&mut self, cx: &mut Context<Self>) {
        self.status = WizardStatus::Testing;
        let config = self.config(cx);
        let task = match &config {
            ConnectionConfig::Postgres(cfg) => PgConnection::test(cfg, cx),
            ConnectionConfig::MongoDB(cfg) => MongoConnection::test(cfg, cx),
            ConnectionConfig::SQLite(cfg) => SqliteConnection::test(cfg, cx),
            ConnectionConfig::Redis(cfg) => RedisConnection::test(cfg, cx),
        };
        cx.spawn(async move |this, cx| {
            let result = task.await;
            cx.update(|cx| apply_test_result(this, result, cx));
        })
        .detach();
    }

    pub(super) fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn reconnect_saved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn browse_sqlite(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn browse_ssh_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn busy(&self) -> bool {
        matches!(
            self.status,
            WizardStatus::Testing | WizardStatus::Connecting | WizardStatus::Saving
        )
    }
}

pub(super) fn apply_test_result(
    this: gpui_kit::WeakEntity<ConnectionWizardPanel>,
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

pub(super) fn drop_opened(opened: OpenedConnection) {
    match opened {
        OpenedConnection::Postgres(conn) => db::close_pg_pool(conn.pool),
        OpenedConnection::Sqlite(conn) => db::close_sqlite_pool(conn.pool),
        OpenedConnection::MongoDB(_) | OpenedConnection::Redis(_) => {}
    }
}
