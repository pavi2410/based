//! Rendering: toolbar, start screen, run marks, results/messages/explain, layout.

use super::*;

impl QueryEditorPanel {
    pub(super) fn render_write_confirm(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (message, confirm) = match self.hold_reason()? {
            HoldReason::ProductionWrite => (
                format!(
                    "This runs {} on {} (production). Run it?",
                    self.held_summary(),
                    self.production_name(cx)?
                ),
                "Run on production",
            ),
            HoldReason::Destructive => (
                format!(
                    "This runs {}, which can't be undone. Run it?",
                    self.held_summary()
                ),
                "Run anyway",
            ),
            HoldReason::AiWrite => return None,
        };
        let id = self.source.id();
        let danger = cx.theme().danger;
        Some(
            h_flex()
                .gap_2()
                .px_2()
                .py_1p5()
                .items_center()
                .border_b_1()
                .border_color(danger.opacity(0.4))
                .bg(danger.opacity(0.08))
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .small()
                        .text_color(danger),
                )
                .child(div().flex_1().text_sm().child(message))
                .child(
                    Button::new(format!("{id}-write-run"))
                        .danger()
                        .small()
                        .label(confirm)
                        .on_click(cx.listener(|this, _, _, cx| this.confirm_write_run(cx))),
                )
                .child(
                    Button::new(format!("{id}-write-cancel"))
                        .ghost()
                        .small()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.clear_hold(cx))),
                )
                .into_any_element(),
        )
    }

    /// Nothing has run yet and no result is loaded.
    pub(super) fn shows_start_screen(&self, cx: &App) -> bool {
        matches!(self.status, QueryStatus::Idle) && self.result.read(cx).delegate().rows.is_empty()
    }

    /// First screen of a fresh query tab: how to run, and recent queries to restore.
    pub(super) fn render_start_screen(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (border, muted, fg, hover) = (
            theme.border,
            theme.muted_foreground,
            theme.foreground,
            theme.muted.opacity(0.4),
        );
        let recent: Vec<(String, String)> = cx
            .try_global::<QueryStore>()
            .map(|store| {
                store
                    .history_for(&self.conn_id)
                    .into_iter()
                    .take(4)
                    .map(|h| {
                        let first_line = h.query.lines().next().unwrap_or("").trim().to_string();
                        let meta = format!(
                            "{} · {} ms",
                            format_relative(h.ran_at, OffsetDateTime::now_utc()),
                            h.duration.as_millis()
                        );
                        (first_line, meta)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let full: Vec<String> = cx
            .try_global::<QueryStore>()
            .map(|store| {
                store
                    .history_for(&self.conn_id)
                    .into_iter()
                    .take(4)
                    .map(|h| h.query.clone())
                    .collect()
            })
            .unwrap_or_default();
        let id = self.source.id();
        v_flex()
            .flex_1()
            .min_h(px(0.0))
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Run a statement to see results"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Press the Run button, or Cmd-Enter in the editor."),
                    ),
            )
            .when(!recent.is_empty(), |v| {
                v.child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(muted)
                                .child("RECENT ON THIS CONNECTION"),
                        )
                        .child(
                            v_flex()
                                .max_w(px(560.0))
                                .rounded(px(8.0))
                                .border_1()
                                .border_color(border)
                                .overflow_hidden()
                                .children(recent.into_iter().enumerate().map(
                                    |(i, (line, meta))| {
                                        let sql = full[i].clone();
                                        h_flex()
                                            .id(SharedString::from(format!("{id}-recent-{i}")))
                                            .gap_3()
                                            .px_3()
                                            .py_1p5()
                                            .items_center()
                                            .cursor_pointer()
                                            .when(i > 0, |r| r.border_t_1().border_color(border))
                                            .hover(move |s| s.bg(hover))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_xs()
                                                    .font_family(prefs::code_font_family(cx))
                                                    .text_color(fg)
                                                    .child(line),
                                            )
                                            .child(div().text_xs().text_color(muted).child(meta))
                                            .on_click(cx.listener(move |panel, _, window, cx| {
                                                panel.set_sql(&sql, window, cx);
                                            }))
                                    },
                                )),
                        ),
                )
            })
            .into_any_element()
    }

    /// A run mark beside the first line of every statement, placed from the editor's
    /// last layout (scroll offset, line height, wrapped rows).
    pub(super) fn render_run_marks(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.status.is_running() {
            return Vec::new();
        }
        let st = self.sql_input.read(cx);
        let (Some(text_bounds), Some(line_height)) = (st.text_bounds(), st.line_height()) else {
            return Vec::new();
        };
        // `text_bounds` already moves with the scroll offset, so it is not added again.
        let view = st.input_bounds();
        let left = st.input_bounds().origin.x + px(3.0);
        let text = st.value().to_string();
        let marks: Vec<(usize, Range<usize>, Pixels)> =
            split_statements(&text, self.source.engine())
                .into_iter()
                .enumerate()
                .filter_map(|(i, range)| {
                    let line = text[..range.start].matches('\n').count();
                    let row = st.display_map().buffer_line_to_display_row(line);
                    let y = text_bounds.origin.y + line_height * row as f32;
                    let inside =
                        y >= view.top() - px(1.0) && y + line_height <= view.bottom() + px(1.0);
                    inside.then_some((i, range, y))
                })
                .collect();
        let (success, hover) = (cx.theme().success_foreground, cx.theme().muted.opacity(0.6));
        let id = self.source.id();
        marks
            .into_iter()
            .map(|(i, range, y)| {
                deferred(
                    anchored()
                        .position(point(left, y + (line_height - px(16.0)) / 2.0))
                        .anchor(Anchor::TopLeft)
                        .child(
                            div()
                                .id(SharedString::from(format!("{id}-run-mark-{i}")))
                                .size(px(16.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover))
                                .tooltip(|window, cx| {
                                    Tooltip::new("Run this statement").build(window, cx)
                                })
                                .child(Icon::new(IconName::Play).xsmall().text_color(success))
                                .on_click(cx.listener(move |panel, _, _, cx| {
                                    panel.run_statement(range.clone(), cx);
                                })),
                        ),
                )
                .with_priority(500)
                .into_any_element()
            })
            .collect()
    }

    pub(super) fn render_bottom_body(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.bottom_tab {
            BottomTab::Results if self.shows_start_screen(cx) => self.render_start_screen(cx),
            BottomTab::Results => {
                let running = self.status.is_running();
                let empty = self.result.read(cx).delegate().rows.is_empty();
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.0))
                    .when(running && !empty, |d| d.opacity(0.55))
                    .child(render_row_table(&self.result, cx))
                    .when(running && empty, |d| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .child(motion::skeleton_table(6, 4, cx)),
                        )
                    })
                    .into_any_element()
            }
            BottomTab::Messages => self.render_messages(cx),
            BottomTab::Explain => self.render_explain(cx),
        }
    }

    pub(super) fn render_messages(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        match &self.status {
            QueryStatus::Error { message: full, .. } => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .child(query_error_card(
                    format!("{}-query-error-card", self.source.id()),
                    full.clone().into(),
                    Some(self.error_actions(full, cx)),
                    cx,
                ))
                .into_any_element(),
            QueryStatus::Done {
                rows,
                affected,
                elapsed_ms,
                ..
            } => {
                if self.last_run.log.len() > 1 {
                    return v_flex()
                        .flex_1()
                        .min_h(px(0.0))
                        .p_3()
                        .gap_1()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_xs()
                        .text_color(muted)
                        .children(self.last_run.log.iter().map(|l| div().child(l.clone())))
                        .into_any_element();
                }
                let affected = affected.map_or(String::new(), |a| format!(" · {a} affected"));
                hint(
                    format!("Query OK · {rows} rows{affected} · {elapsed_ms} ms"),
                    muted,
                )
            }
            QueryStatus::Running(_) => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_xs()
                .text_color(muted)
                .child(ShimmerText::new("Running…"))
                .into_any_element(),
            QueryStatus::Cancelled { after_ms } => hint(
                format!(
                    "Stopped after {:.1} s. The server may still finish the statement; nothing is rolled back.",
                    *after_ms as f32 / 1000.0
                ),
                muted,
            ),
            QueryStatus::Idle => hint("No messages yet. Run a query to see output here.", muted),
        }
    }

    /// Location text plus Jump to error and Fix with AI for the error card.
    pub(super) fn error_actions(&self, full: &str, cx: &mut Context<Self>) -> AnyElement {
        let id = self.source.id();
        let muted = cx.theme().muted_foreground;
        let error_span = self.status.error_span();
        let place = error_span.as_ref().map(|span| {
            let (line, col) = line_col(&self.current_sql(cx), span.start);
            format!("Line {line}, column {col}")
        });
        let _ = full;
        h_flex()
            .gap(px(8.0))
            .items_center()
            .children(place.map(|p| div().text_xs().text_color(muted).child(p)))
            .when(error_span.is_some(), |h| {
                h.child(
                    Button::new(format!("{id}-jump-error"))
                        .ghost()
                        .xsmall()
                        .label("Jump to error")
                        .on_click(cx.listener(|panel, _, window, cx| {
                            panel.jump_to_error(window, cx);
                        })),
                )
            })
            .child(
                Button::new(format!("{id}-fix-ai"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Bot)
                    .label("Fix with AI")
                    .on_click(cx.listener(|panel, _, window, cx| {
                        panel.fix_with_ai(window, cx);
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_explain(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        match &self.explain {
            ExplainView::Empty => {
                hint("Click Explain in the toolbar to see the query plan.", muted)
            }
            ExplainView::Running => hint("Running EXPLAIN…", muted),
            ExplainView::Plan(plan) => div()
                .flex_1()
                .min_h(px(0.0))
                .child(plan.render(theme))
                .into_any_element(),
            ExplainView::Text(text) => div()
                .flex_1()
                .min_h(px(0.0))
                .p_3()
                .text_sm()
                .font_family(theme.mono_font_family.clone())
                .text_color(theme.foreground)
                .child(text.clone())
                .into_any_element(),
        }
    }

    pub(super) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = self.source.id();
        let project_dir = cx.try_global::<ProjectRoot>().map(|p| p.0.clone());
        let var_map = cx.global::<ProjectVars>().vars.clone();
        let mono_font = cx.theme().mono_font_family.clone();

        h_flex()
            .gap(px(6.0))
            .px_2()
            .py(px(4.0))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
            .child(if self.status.is_running() {
                Button::new(format!("{id}-stop"))
                    .danger()
                    .small()
                    .icon(IconName::Close)
                    .label("Stop")
                    .on_click(cx.listener(|panel, _, _, cx| panel.stop(cx)))
                    .into_any_element()
            } else {
                let panel = cx.weak_entity();
                DropdownButton::new(format!("{id}-run-split"))
                    .primary()
                    .small()
                    .button(
                        Button::new(format!("{id}-run"))
                            .icon(IconName::Play)
                            .label("Run")
                            .child(shortcut_run_kbd_in_primary_button(cx))
                            .on_click(cx.listener(|panel, _, _, cx| panel.run(cx))),
                    )
                    .menu_accessibility_label("Run options")
                    .dropdown_menu(move |menu, _, _| {
                        let (a, b) = (panel.clone(), panel.clone());
                        menu.item(
                            PopupMenuItem::new("Run Selection or Current Statement")
                                .icon(IconName::Play)
                                .on_click(move |_, _, cx| {
                                    let _ = a.update(cx, |p, cx| p.run(cx));
                                }),
                        )
                        .item(
                            PopupMenuItem::new("Run All Statements")
                                .icon(IconName::Play)
                                .on_click(move |_, _, cx| {
                                    let _ = b.update(cx, |p, cx| p.run_all(cx));
                                }),
                        )
                    })
                    .into_any_element()
            })
            .child(
                Button::new(format!("{id}-format"))
                    .ghost()
                    .small()
                    .label("Format")
                    .on_click(cx.listener(|panel, _, window, cx| panel.format(window, cx))),
            )
            .child(
                Button::new(format!("{id}-explain"))
                    .ghost()
                    .small()
                    .label("Explain")
                    .on_click(cx.listener(|panel, _, _, cx| panel.switch_to_explain(cx))),
            )
            .child(
                Button::new(format!("{id}-ai"))
                    .ghost()
                    .small()
                    .icon(IconName::Bot)
                    .label("Ask AI")
                    .selected(self.ai.is_open())
                    .on_click(cx.listener(|panel, _, window, cx| panel.toggle_ai(window, cx))),
            )
            .child(self.assist_menu(cx))
            .child(self.history_menu(cx))
            .child(
                Button::new(format!("{id}-save"))
                    .small()
                    .label("Save")
                    .on_click(cx.listener(|panel, _, window, cx| panel.save_query(window, cx))),
            )
            .child(div().flex_1())
            .child(query_panel_extras::variables_popover(
                format!("{id}-vars-popover"),
                project_dir,
                var_map,
                mono_font,
                cx,
            ))
    }
}

pub(super) fn status_display(status: &QueryStatus) -> QueryStatusDisplay {
    match status {
        QueryStatus::Idle => QueryStatusDisplay::Idle,
        QueryStatus::Running(_) => QueryStatusDisplay::Running,
        QueryStatus::Done {
            rows,
            affected,
            elapsed_ms,
            finished_at,
        } => QueryStatusDisplay::Done {
            rows: *rows,
            affected: *affected,
            elapsed_ms: *elapsed_ms,
            finished_at: *finished_at,
        },
        QueryStatus::Error { message, .. } => QueryStatusDisplay::Error(message.clone().into()),
        QueryStatus::Cancelled { after_ms } => QueryStatusDisplay::Cancelled {
            after_ms: *after_ms,
        },
    }
}

pub(super) fn hint(text: impl Into<SharedString>, muted: Hsla) -> AnyElement {
    div()
        .flex_1()
        .min_h(px(0.0))
        .p_3()
        .text_xs()
        .text_color(muted)
        .child(text.into())
        .into_any_element()
}

impl Render for QueryEditorPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_gutter(cx);
        let id = self.source.id();
        let has_error = matches!(self.status, QueryStatus::Error { .. });
        let toolbar = self.render_toolbar(cx);
        let ai_confirm = self.render_ai_confirm(cx);
        let write_confirm = self.render_write_confirm(cx);
        let run_marks = self.render_run_marks(cx);
        let editor_pane = div().size_full().p_2().child(sql_editor::code_editor_flex(
            &self.sql_input,
            has_error,
            false,
            cx,
        ));

        let on_select: Rc<dyn Fn(StripTab, &mut Window, &mut App)> = {
            let entity = cx.entity();
            Rc::new(move |tab, _, cx| {
                entity.update(cx, |panel, cx| {
                    match tab {
                        StripTab::Result(i) => {
                            panel.bottom_tab = BottomTab::Results;
                            panel.show_result(i, cx);
                        }
                        StripTab::Messages => panel.bottom_tab = BottomTab::Messages,
                        StripTab::Explain => panel.bottom_tab = BottomTab::Explain,
                    }
                    cx.notify();
                });
            })
        };
        let (headers, rows): (Vec<String>, Vec<Vec<Cell>>) = {
            let d = self.result.read(cx).delegate();
            (
                d.columns.iter().map(|c| c.key.to_string()).collect(),
                d.rows.clone(),
            )
        };
        let result_counts: Vec<usize> = match (self.last_run.results.len(), &self.status) {
            (0, QueryStatus::Done { rows, .. }) if *rows > 0 => vec![*rows],
            _ => self.last_run.results.iter().map(|r| r.rows.len()).collect(),
        };
        let trailing = h_flex()
            .gap(px(10.0))
            .items_center()
            .child(query_result_meta(
                &status_display(&self.status),
                self.status.running_ms(),
                cx,
            ))
            .children(self.can_ask_about_results(cx).then(|| {
                Button::new(format!("{id}-ask-results"))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Bot)
                    .label("Ask about results")
                    .tooltip("Send the first rows to the assistant")
                    .on_click(cx.listener(|panel, _, _, cx| panel.assist(Assist::Results, cx)))
            }))
            .child(export_popover(&format!("{id}-qe"), headers, rows))
            .into_any_element();
        let strip = result_tab_strip(
            &format!("{id}-bt"),
            self.bottom_tab,
            has_error,
            result_counts,
            self.last_run.active,
            Some(trailing),
            on_select,
            cx,
        );
        let bottom_pane = v_flex()
            .size_full()
            .child(strip)
            .child(self.render_bottom_body(cx));

        let crumbs = tab_breadcrumb_for_connection(&self.conn_id, ["Query"], cx);
        let footer = tab_breadcrumb_footer(format!("{id}-qe-breadcrumb"), crumbs, None, cx);

        panel_tab_content(
            v_flex()
                .w_full()
                .h_full()
                .min_h(px(0.0))
                .on_action(cx.listener(|panel, _: &AskAi, window, cx| panel.toggle_ai(window, cx)))
                .children(run_marks)
                .child(toolbar)
                .child(motion::glimmer_bar(
                    format!("{id}-run-glimmer"),
                    self.status.is_running(),
                    cx,
                ))
                .when_some(self.ai.bar(), |v, bar| v.child(bar))
                .children(ai_confirm)
                .children(write_confirm)
                .child(
                    div().flex_1().min_h(px(0.0)).child(
                        v_resizable(SharedString::from(format!("{id}-query-split")))
                            .with_state(&self.split_state)
                            .child(resizable_panel().size(px(220.0)).child(editor_pane))
                            .child(resizable_panel().child(bottom_pane)),
                    ),
                ),
            footer,
        )
    }
}
