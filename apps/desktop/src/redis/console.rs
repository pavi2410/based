// redis::console — raw command REPL (newest output first).

use based_redis::{Conn, exec, is_dangerous};
use gpui_kit::component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::{Editor, EditorState, InputEvent},
    menu::PopupMenu,
    scroll::ScrollableElement,
    v_flex,
};
use gpui_kit::{prelude::*, *};

use crate::app::prefs;
use crate::db;
use crate::editor::redis_completion::RedisCompletionProvider;
use crate::widgets::sql_editor::{set_input_text, text_from_input};
use crate::workspace::pop_out::PopOutWindowTitle;

struct Output {
    command: String,
    result: Result<String, String>,
}

pub struct ConsolePanel {
    focus_handle: FocusHandle,
    conn: Conn,
    input: Entity<EditorState>,
    outputs: Vec<Output>,
    /// Dangerous command awaiting a second Enter.
    pending: Option<String>,
    running: bool,
    pub(crate) tab_label: SharedString,
}

impl ConsolePanel {
    pub fn new(
        conn: Conn,
        initial: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            let mut state = EditorState::new(window, cx)
                .line_number(false)
                .searchable(false)
                .submit_on_enter(true);
            state.lsp_mut().completion_provider = Some(RedisCompletionProvider::new());
            state
        });
        input.update(cx, |s, cx| {
            s.set_value(initial.as_deref().unwrap_or(""), window, cx)
        });
        cx.subscribe_in(&input, window, |panel, _, event, window, cx| {
            if let InputEvent::PressEnter {
                secondary: false,
                shift: false,
            } = event
            {
                panel.submit(window, cx);
            }
        })
        .detach();
        Self {
            focus_handle: cx.focus_handle(),
            conn,
            input,
            outputs: Vec::new(),
            pending: None,
            running: false,
            tab_label: "Console".into(),
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let line = text_from_input(&self.input, cx).trim().to_string();
        if line.is_empty() || self.running {
            return;
        }
        if is_dangerous(&line) && self.pending.as_deref() != Some(line.as_str()) {
            self.pending = Some(line);
            cx.notify();
            return;
        }
        self.pending = None;
        self.running = true;
        cx.notify();

        let mut conn = self.conn.clone();
        let cmd = line.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = db::run(cx, async move { exec(&mut conn, &cmd).await }).await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |panel, cx| {
                    panel.running = false;
                    panel.outputs.insert(
                        0,
                        Output {
                            command: line,
                            result: result.map_err(|e| format!("(error) {e}")),
                        },
                    );
                    set_input_text(&panel.input, "", window, cx);
                    cx.notify();
                })
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for ConsolePanel {}

impl Focusable for ConsolePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for ConsolePanel {
    crate::based_panel_behavior!("RedisConsole");
}

impl Panel for ConsolePanel {
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

impl PopOutWindowTitle for ConsolePanel {
    fn pop_out_window_title(&mut self, _: &mut Window, _: &mut App) -> String {
        "Redis Console".into()
    }
}

impl Render for ConsolePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let danger = cx.theme().danger;
        let border = cx.theme().border.opacity(0.6);
        let mono = prefs::code_font_family(cx);

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .p_2()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex_1()
                            .h(px(64.0))
                            .font_family(mono.clone())
                            .child(Editor::new(&self.input).h_full()),
                    )
                    .child(
                        Button::new("redis-run")
                            .primary()
                            .icon(IconName::Play)
                            .label(if self.running { "Running…" } else { "Run" })
                            .on_click(cx.listener(|p, _, w, cx| p.submit(w, cx))),
                    ),
            )
            .when_some(self.pending.clone(), |v, cmd| {
                v.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(danger)
                        .child(format!(
                            "`{cmd}` is destructive. Press Enter again to run it."
                        )),
                )
            })
            .child(
                v_flex()
                    .id("redis-output")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .font_family(mono)
                    .text_sm()
                    .children(self.outputs.iter().map(|o| {
                        let (text, color) = match &o.result {
                            Ok(t) => (t.clone(), cx.theme().foreground),
                            Err(e) => (e.clone(), danger),
                        };
                        v_flex()
                            .gap_1()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(border)
                            .child(div().text_color(muted).child(format!("> {}", o.command)))
                            .child(div().text_color(color).child(text))
                    })),
            )
    }
}
