//! Tool calls the pane runs itself: choosing, opening and switching databases.

use based_ai::context::ContextLevel;
use based_ai::tools::{CONNECT, LIST_CONNECTIONS, ToolCall, ToolResult};
use gpui_kit::{Context, Entity, Task};
use std::fmt::Write as _;

use super::{ChatContext, ChatPane};
use crate::connection::{ConnectionEntry, ConnectionState, open_connection, opened_into_any};
use crate::db;

const NO_DB: &str = "no database is active; call list_connections, then connect";

fn level_text(level: ContextLevel) -> &'static str {
    match level {
        ContextLevel::Off => "assistant access off",
        ContextLevel::Schema => "schema only",
        ContextLevel::SchemaRows => "schema and rows",
    }
}

impl ChatPane {
    /// Starts one tool call and returns its result.
    pub(super) fn start_call(
        &mut self,
        call: &ToolCall,
        cx: &mut Context<Self>,
    ) -> Task<ToolResult> {
        match call.name.as_str() {
            LIST_CONNECTIONS => Task::ready(self.list_connections(cx)),
            CONNECT => self.connect(call, cx),
            _ => self.db_call(call, cx),
        }
    }

    fn db_call(&mut self, call: &ToolCall, cx: &mut Context<Self>) -> Task<ToolResult> {
        let (Some(tools), Some(active)) = (self.tools.clone(), self.active()) else {
            return Task::ready(ToolResult::err(NO_DB));
        };
        self.targets.insert(call.id.clone(), active.label.clone());
        let call = call.clone();
        cx.spawn(async move |_, cx| {
            db::run_infallible(cx, async move { tools.call(&call).await })
                .await
                .unwrap_or_else(|e| ToolResult::err(format!("{e:#}")))
        })
    }

    fn list_connections(&self, cx: &Context<Self>) -> ToolResult {
        let active = self.active().map(|c| c.entry.clone());
        let mut out = String::new();
        for entity in self.registry.read(cx).connections() {
            let entry = entity.read(cx);
            let level = ChatContext::of(entity, cx).level;
            let _ = writeln!(
                out,
                "{} | {} | {} | {}{}",
                entry.config.label(),
                entry.engine().short_label(),
                entry.state.label(),
                level_text(level),
                if active.as_ref() == Some(entity) {
                    " | active"
                } else {
                    ""
                },
            );
        }
        if out.is_empty() {
            return ToolResult::err("no connections are configured in this project");
        }
        ToolResult::ok(out)
    }

    fn find(&self, name: &str, cx: &Context<Self>) -> Option<Entity<ConnectionEntry>> {
        let conns = self.registry.read(cx).connections();
        let by = |exact: bool| {
            conns.iter().find(|e| {
                let label = e.read(cx).config.label();
                if exact {
                    label == name
                } else {
                    label.eq_ignore_ascii_case(name)
                }
            })
        };
        by(true).or_else(|| by(false)).cloned()
    }

    fn connect(&mut self, call: &ToolCall, cx: &mut Context<Self>) -> Task<ToolResult> {
        let Some(name) = call.args["connection"].as_str() else {
            return Task::ready(ToolResult::err("missing string argument `connection`"));
        };
        let Some(entry) = self.find(name, cx) else {
            return Task::ready(ToolResult::err(format!(
                "no connection named `{name}`; call list_connections"
            )));
        };
        if ChatContext::of(&entry, cx).level == ContextLevel::Off {
            return Task::ready(ToolResult::err(
                "the assistant is not allowed to use this connection (`[ai]` is off in its config)",
            ));
        }
        match entry.read(cx).state {
            ConnectionState::Connected(_) => return Task::ready(self.pin(&entry, cx)),
            ConnectionState::Connecting => {
                return Task::ready(ToolResult::err("still connecting; try again shortly"));
            }
            ConnectionState::Disconnected | ConnectionState::Failed { .. } => {}
        }
        let config = entry.read(cx).config.clone();
        entry.update(cx, |e, cx| {
            e.state = ConnectionState::Connecting;
            e.last_error = None;
            cx.notify();
        });
        let open = open_connection(config, cx);
        cx.spawn(async move |this, cx| {
            let result = open.await;
            let failure = cx.update(|app| {
                entry.update(app, |e, ecx| match result {
                    Ok(opened) => {
                        e.state = ConnectionState::Connected(opened_into_any(opened, ecx));
                        None
                    }
                    Err(err) => {
                        e.state = ConnectionState::Failed {
                            reason: err.to_string(),
                        };
                        e.last_error = Some(err.to_string());
                        ecx.notify();
                        Some(format!("{err:#}"))
                    }
                })
            });
            if let Some(err) = failure {
                return ToolResult::err(format!("could not connect: {err}"));
            }
            this.update(cx, |this, cx| this.pin(&entry, cx))
                .unwrap_or_else(|_| ToolResult::err("the assistant was closed"))
        })
    }

    /// Makes `entry` the connection the other tools use.
    fn pin(&mut self, entry: &Entity<ConnectionEntry>, cx: &mut Context<Self>) -> ToolResult {
        let ctx = ChatContext::of(entry, cx);
        let rows = if ctx.level == ContextLevel::SchemaRows {
            "Row queries are allowed."
        } else {
            "Schema only: row queries are disabled."
        };
        let msg = format!(
            "Now using {} ({}). {rows}",
            ctx.label,
            ctx.engine.short_label()
        );
        self.pinned = Some(ctx);
        self.tools = self.connection_tools(cx);
        self.ensure_tables(cx);
        cx.notify();
        ToolResult::ok(msg)
    }
}
