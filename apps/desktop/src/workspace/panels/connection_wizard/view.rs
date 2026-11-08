use super::*;

impl Render for ConnectionWizardPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.stage {
            Stage::Pick => self.render_picker(cx),
            Stage::Form => self.render_form(cx).into_any_element(),
        }
    }
}

impl ConnectionWizardPanel {
    fn render_form(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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

        let footer = v_flex()
            .w_full()
            .flex_shrink_0()
            .gap_2()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .when(show_status, |v| {
                v.child(status_card(&self.status, status, cx))
            })
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("wizard-test")
                            .label("Test")
                            .disabled(busy)
                            .on_click(cx.listener(|panel, _, _, cx| panel.test_connection(cx))),
                    )
                    .child(
                        Button::new("wizard-save")
                            .when(editing, |b| b.primary())
                            .label("Save")
                            .disabled(busy)
                            .on_click(cx.listener(|panel, _, window, cx| panel.save(window, cx))),
                    )
                    .when(!editing, |h| {
                        h.child(
                            Button::new("wizard-connect")
                                .primary()
                                .label("Connect")
                                .disabled(busy)
                                .on_click(
                                    cx.listener(|panel, _, window, cx| panel.connect(window, cx)),
                                ),
                        )
                    }),
            );

        v_flex().size_full().child(
            div().flex_1().min_h_0().child(
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
                            h_flex()
                                .gap_2()
                                .items_center()
                                .when(!editing, |h| {
                                    h.child(
                                        Button::new("wizard-back")
                                            .ghost()
                                            .small()
                                            .icon(Icon::new(IconName::ChevronLeft))
                                            .tooltip("Choose a different database")
                                            .on_click(cx.listener(|panel, _, _, cx| {
                                                panel.stage = Stage::Pick;
                                                cx.notify();
                                            })),
                                    )
                                })
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(heading),
                                ),
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
                    "Name",
                    muted,
                    Input::new(&self.name)
                        .cleanable(true)
                        .aria_label("Connection name"),
                ))
                .child(form_section("Connection", cx))
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
                .child(form_section("Identity", cx))
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_xs().text_color(muted).child("Environment"))
                        .child(h_flex().gap_1().children(Environment::ALL.map(|env| {
                            let active = Environment::from_tags(&self.tags) == Some(env);
                            Button::new(SharedString::from(format!("wizard-env-{}", env.tag())))
                                .small()
                                .when(active, |b| b.primary())
                                .when(!active, |b| b.outline())
                                .label(env.label())
                                .on_click(cx.listener(move |panel, _, _, cx| {
                                    panel.tags.retain(|t| Environment::from_tag(t).is_none());
                                    if !active {
                                        panel.tags.push(env.tag().to_string());
                                    }
                                    cx.notify();
                                }))
                        })))
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("Marks the connection in the rail. Saved as a tag."),
                        ),
                )
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
        )))
        .child(footer)
    }
}

/// Pinned result strip: tone follows the status (busy, ok, error).
fn status_card(status: &WizardStatus, message: SharedString, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let (tone, bg) = match status {
        WizardStatus::TestOk { .. } | WizardStatus::ConnectOk | WizardStatus::Saved => {
            (theme.green, theme.green.opacity(0.1))
        }
        WizardStatus::TestErr(_) | WizardStatus::ConnectErr(_) | WizardStatus::SaveErr(_) => {
            (theme.red, theme.red.opacity(0.1))
        }
        _ => (theme.muted_foreground, theme.muted.opacity(0.4)),
    };
    let busy = matches!(
        status,
        WizardStatus::Testing | WizardStatus::Connecting | WizardStatus::Saving
    );
    h_flex()
        .w_full()
        .gap_2()
        .items_start()
        .px_3()
        .py_2()
        .rounded(px(8.0))
        .border_1()
        .border_color(tone.opacity(0.35))
        .bg(bg)
        .child(div().mt(px(5.0)).size(px(8.0)).rounded_full().bg(tone))
        .child(div().flex_1().text_sm().child(message))
        .when(busy, |d| d.opacity(0.9))
}
