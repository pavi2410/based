// redis::key_view — inspect and edit a single key (value, TTL, entries).

use std::future::Future;

use based_redis::{
    Conn, KeyMeta, KeyType, KeyValue, PAGE, add_entry, delete_key, key_meta, read_value,
    remove_entry, set_string, set_ttl,
};
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::PopupMenu,
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;
use crate::db;
use crate::widgets::panel_status::PanelStatus;
use crate::widgets::sql_editor::{
    code_editor_flex, new_code_input, set_input_text, text_from_input,
};
use crate::widgets::{metadata_pill, new_field, set_field};
use crate::workspace::pop_out::PopOutWindowTitle;
use gpui_kit::component::input::EditorState;

pub struct KeyViewPanel {
    focus_handle: FocusHandle,
    conn: Conn,
    key: String,
    meta: Option<KeyMeta>,
    value: KeyValue,
    loaded: bool,
    editor: Entity<EditorState>,
    ttl_input: Entity<InputState>,
    add_input: Entity<InputState>,
    confirm_delete: bool,
    clear_add_on_ok: bool,
    status: PanelStatus,
    pub(crate) tab_label: SharedString,
}

impl KeyViewPanel {
    pub fn new(conn: Conn, key: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let add_input = new_field(window, cx, "", "");
        cx.subscribe_in(&add_input, window, |panel, _, event, window, cx| {
            if let InputEvent::PressEnter {
                secondary: false,
                shift: false,
            } = event
            {
                panel.add(window, cx);
            }
        })
        .detach();
        let mut panel = Self {
            focus_handle: cx.focus_handle(),
            conn,
            tab_label: key.clone().into(),
            key,
            meta: None,
            value: KeyValue::Unsupported,
            loaded: false,
            editor: new_code_input("text", "", window, cx),
            ttl_input: new_field(window, cx, "", "TTL seconds"),
            add_input,
            confirm_delete: false,
            clear_add_on_ok: false,
            status: PanelStatus::busy("Loading…"),
        };
        panel.reload(window, cx);
        panel
    }

    fn kind(&self) -> Option<KeyType> {
        self.meta.as_ref().and_then(|m| m.kind)
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut conn = self.conn.clone();
        let key = self.key.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = db::run(cx, async move {
                let meta = key_meta(&mut conn, &key).await?;
                let value = match meta.kind {
                    Some(kind) => read_value(&mut conn, &key, kind).await?,
                    None => KeyValue::Unsupported,
                };
                Ok((meta, value))
            })
            .await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |panel, cx| panel.apply_loaded(result, window, cx))
            });
        })
        .detach();
    }

    fn apply_loaded(
        &mut self,
        result: anyhow::Result<(KeyMeta, KeyValue)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_delete = false;
        match result {
            Ok((meta, value)) => {
                if let KeyValue::Text(text) = &value {
                    set_input_text(&self.editor, text, window, cx);
                }
                if self.clear_add_on_ok {
                    set_field(&self.add_input, "", window, cx);
                }
                self.meta = Some(meta);
                self.value = value;
                self.status = PanelStatus::Idle;
            }
            Err(e) => {
                self.status = PanelStatus::error(format!("{e:#}"));
            }
        }
        self.clear_add_on_ok = false;
        self.loaded = true;
        cx.notify();
    }

    /// Run a mutation off-thread, then reload so the view reflects the server.
    fn act<F, Fut>(
        &mut self,
        status: &str,
        clear_add: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: F,
    ) where
        F: FnOnce(Conn, String) -> Fut + Send + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        self.status = PanelStatus::busy(status.to_string());
        cx.notify();
        let fut = f(self.conn.clone(), self.key.clone());
        cx.spawn_in(window, async move |this, cx| {
            let result = db::run(cx, fut).await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |panel, cx| match result {
                    Ok(()) => {
                        panel.clear_add_on_ok = clear_add;
                        panel.reload(window, cx);
                    }
                    Err(e) => {
                        panel.status = PanelStatus::error(format!("{e:#}"));
                        cx.notify();
                    }
                })
            });
        })
        .detach();
    }

    fn save_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = text_from_input(&self.editor, cx);
        self.act("Saving…", false, window, cx, move |mut c, k| async move {
            set_string(&mut c, &k, &text).await
        });
    }

    fn apply_ttl(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = self.ttl_input.read(cx).value().to_string();
        match raw.trim().parse::<i64>() {
            Ok(secs) if secs > 0 => {
                self.act(
                    "Setting TTL…",
                    false,
                    window,
                    cx,
                    move |mut c, k| async move { set_ttl(&mut c, &k, Some(secs)).await },
                );
            }
            _ => {
                self.status = PanelStatus::error("Enter a positive TTL in seconds");
                cx.notify();
            }
        }
    }

    fn persist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.act("Removing TTL…", false, window, cx, |mut c, k| async move {
            set_ttl(&mut c, &k, None).await
        });
    }

    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.confirm_delete {
            self.confirm_delete = true;
            cx.notify();
            return;
        }
        self.act("Deleting…", false, window, cx, |mut c, k| async move {
            delete_key(&mut c, &k).await
        });
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.kind() else { return };
        let text = self.add_input.read(cx).value().to_string();
        self.act("Adding…", true, window, cx, move |mut c, k| async move {
            add_entry(&mut c, &k, kind, &text).await
        });
    }

    fn remove(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.kind() else { return };
        self.act("Removing…", false, window, cx, move |mut c, k| async move {
            remove_entry(&mut c, &k, kind, &id).await
        });
    }

    fn add_hint(kind: KeyType) -> &'static str {
        match kind {
            KeyType::Hash => "field value",
            KeyType::ZSet => "score member",
            KeyType::List => "value to append",
            _ => "member",
        }
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let meta = self.meta.as_ref();
        let ttl = match meta.and_then(|m| m.ttl) {
            Some(s) => format!("{s}s"),
            None => "none".to_string(),
        };
        let kind = meta
            .and_then(|m| m.kind)
            .map(|k| k.label())
            .unwrap_or("missing");
        let len = meta.and_then(|m| m.len).map(|n| n.to_string());
        let memory = meta.and_then(|m| m.memory).map(|n| format!("{n} B"));
        let delete_label = if self.confirm_delete {
            "Confirm delete"
        } else {
            "Delete"
        };
        let exists = self.kind().is_some();

        h_flex()
            .w_full()
            .flex_shrink_0()
            .flex_wrap()
            .gap_2()
            .items_center()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.72))
            .bg(cx.theme().muted.opacity(0.18))
            .child(metadata_pill("type", kind, cx))
            .child(metadata_pill("ttl", ttl, cx))
            .when_some(len, |h, v| h.child(metadata_pill("len", v, cx)))
            .when_some(memory, |h, v| h.child(metadata_pill("mem", v, cx)))
            .child(div().flex_1())
            .when(exists, |h| {
                h.child(
                    div()
                        .w(px(110.0))
                        .child(Input::new(&self.ttl_input).small()),
                )
                .child(
                    Button::new("redis-ttl-set")
                        .small()
                        .label("Set TTL")
                        .on_click(cx.listener(|p, _, w, cx| p.apply_ttl(w, cx))),
                )
                .child(
                    Button::new("redis-ttl-persist")
                        .small()
                        .label("Persist")
                        .on_click(cx.listener(|p, _, w, cx| p.persist(w, cx))),
                )
            })
            .child(
                Button::new("redis-refresh")
                    .small()
                    .icon(IconName::RefreshCw)
                    .label("Refresh")
                    .on_click(cx.listener(|p, _, w, cx| p.reload(w, cx))),
            )
            .when(exists, |h| {
                h.child(
                    Button::new("redis-delete")
                        .small()
                        .danger()
                        .label(delete_label)
                        .on_click(cx.listener(|p, _, w, cx| p.delete(w, cx))),
                )
            })
    }

    fn render_body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let note = |text: &'static str| {
            div()
                .p_4()
                .text_sm()
                .text_color(muted)
                .child(text)
                .into_any_element()
        };
        if !self.loaded {
            return note("Loading…");
        }
        let Some(kind) = self.kind() else {
            return note("Key does not exist (it may have expired or been deleted).");
        };
        match self.value.clone() {
            KeyValue::Text(_) => v_flex()
                .flex_1()
                .min_h_0()
                .child(code_editor_flex(
                    &self.editor,
                    self.status.is_error(),
                    false,
                    cx,
                ))
                .child(
                    h_flex().p_2().justify_end().child(
                        Button::new("redis-save")
                            .primary()
                            .small()
                            .label("Save")
                            .on_click(cx.listener(|p, _, w, cx| p.save_text(w, cx))),
                    ),
                )
                .into_any_element(),
            KeyValue::Entries(rows) => {
                let truncated = rows.len() >= PAGE;
                let border = cx.theme().border.opacity(0.5);
                let mono = prefs::code_font_family(cx);
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_2()
                            .p_2()
                            .child(
                                Input::new(&self.add_input)
                                    .small()
                                    .cleanable(true)
                                    .aria_label(Self::add_hint(kind))
                                    .flex_1(),
                            )
                            .child(
                                Button::new("redis-add")
                                    .small()
                                    .label(format!("Add ({})", Self::add_hint(kind)))
                                    .on_click(cx.listener(|p, _, w, cx| p.add(w, cx))),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("redis-entries")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scrollbar()
                            .font_family(mono)
                            .text_sm()
                            .children(rows.into_iter().enumerate().map(|(ix, row)| {
                                let id = row.id.clone();
                                h_flex()
                                    .gap_3()
                                    .px_2()
                                    .py_1()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(border)
                                    .when(!row.left.is_empty(), |h| {
                                        h.child(
                                            div()
                                                .w(px(160.0))
                                                .flex_shrink_0()
                                                .truncate()
                                                .text_color(muted)
                                                .child(row.left.clone()),
                                        )
                                    })
                                    .child(div().flex_1().min_w_0().child(row.right.clone()))
                                    .child(
                                        Button::new(("redis-rm", ix))
                                            .ghost()
                                            .small()
                                            .icon(IconName::Close)
                                            .on_click(cx.listener(move |p, _, w, cx| {
                                                p.remove(id.clone(), w, cx)
                                            })),
                                    )
                            }))
                            .when(truncated, |v| {
                                v.child(
                                    div()
                                        .p_2()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(format!("Showing the first {PAGE} entries.")),
                                )
                            }),
                    )
                    .into_any_element()
            }
            KeyValue::Unsupported => note("Viewing this key type is not supported yet."),
        }
    }
}

impl EventEmitter<PanelEvent> for KeyViewPanel {}

impl Focusable for KeyViewPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for KeyViewPanel {
    crate::based_panel_behavior!("RedisKeyView");
}

impl Panel for KeyViewPanel {
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

impl PopOutWindowTitle for KeyViewPanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        self.key.clone()
    }
}

impl Render for KeyViewPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(self.render_toolbar(cx))
            .children(self.status.line(cx))
            .child(self.render_body(cx))
    }
}
