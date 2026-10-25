use super::*;

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
                    self.postgres_fields(v, muted, cx)
                })
                .when(engine == EngineKind::MongoDB, |v| {
                    self.mongo_fields(v, muted, cx)
                })
                .when(engine == EngineKind::Redis, |v| {
                    self.redis_fields(v, muted, cx)
                })
                .when(engine == EngineKind::SQLite, |v| {
                    self.sqlite_fields(v, muted, cx)
                })
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("wizard-test")
                                .label("Test")
                                .disabled(busy)
                                .on_click(cx.listener(|panel, _, _, cx| panel.test_connection(cx))),
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
                .child(
                    v_flex()
                        .gap_1()
                        .child(labeled_field(
                            "AI assistant",
                            muted,
                            Select::new(&self.ai_select).w_full(),
                        ))
                        .child(div().text_xs().text_color(muted).child(
                            "What the model may see. Queries are never run without confirmation.",
                        )),
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
