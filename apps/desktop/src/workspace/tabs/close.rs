//! Closing center tabs. User gestures confirm before discarding unsaved edits;
//! programmatic teardown (project switch/close) uses the `force_` variant.

use gpui_kit::component::{
    ActiveTheme, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
};
use gpui_kit::{Context, EntityId, SharedString, Window, prelude::*};

use super::WorkspaceRef;
use crate::workspace::Workspace;
use crate::workspace::dock_utils::{
    active_live_center_panel, panel_entity_id, remove_presentation_panel,
};

/// Which tabs a close request targets. Pinned tabs are never included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseScope {
    Tab(EntityId),
    Active,
    Others(EntityId),
    Left(EntityId),
    Right(EntityId),
    All,
    /// Everything without unsaved edits.
    Clean,
    /// Tabs owned by the `.based/` project (keeps Home and workspace-local connections).
    Project,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseOutcome {
    Closed,
    /// Nothing matched (pinned, unknown, or empty scope).
    Nothing,
    /// Unsaved edits: waiting on [`Workspace::confirm_close_prompt`] or cancel.
    NeedsConfirm,
}

/// A close request held back because some target tabs have unsaved edits.
#[derive(Debug)]
pub struct ClosePrompt {
    tabs: Vec<EntityId>,
    dirty: usize,
    shown: bool,
}

impl ClosePrompt {
    pub fn dirty(&self) -> usize {
        self.dirty
    }

    pub fn tabs(&self) -> &[EntityId] {
        &self.tabs
    }
}

impl Workspace {
    /// Close tabs for a user gesture; asks first when any target has unsaved edits.
    pub fn close_tabs(
        &mut self,
        scope: CloseScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> CloseOutcome {
        let tabs = self.close_targets(scope, window, cx);
        if tabs.is_empty() {
            return CloseOutcome::Nothing;
        }
        let tm = self.tab_manager.read(cx);
        let dirty = tabs
            .iter()
            .filter(|id| tm.tab_for_panel_id(**id).is_some_and(|t| t.dirty))
            .count();
        if dirty == 0 {
            self.close_panels(&tabs, window, cx);
            return CloseOutcome::Closed;
        }
        self.close_prompt = Some(ClosePrompt {
            tabs,
            dirty,
            shown: false,
        });
        cx.notify();
        CloseOutcome::NeedsConfirm
    }

    /// Close without asking (the caller already confirmed, or is tearing down).
    pub fn force_close_tabs(
        &mut self,
        scope: CloseScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tabs = self.close_targets(scope, window, cx);
        self.close_panels(&tabs, window, cx);
    }

    pub fn close_prompt(&self) -> Option<&ClosePrompt> {
        self.close_prompt.as_ref()
    }

    pub fn confirm_close_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prompt) = self.close_prompt.take() {
            // Pins or closes since the prompt opened may have changed the targets.
            let tabs: Vec<_> = prompt
                .tabs
                .into_iter()
                .filter(|id| self.can_close_center_panel(*id, cx))
                .collect();
            self.close_panels(&tabs, window, cx);
        }
    }

    pub fn cancel_close_prompt(&mut self, cx: &mut Context<Self>) {
        if self.close_prompt.take().is_some() {
            cx.notify();
        }
    }

    /// Open the confirm dialog for a pending prompt (called while rendering).
    pub(crate) fn show_close_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.close_prompt.as_mut() else {
            return;
        };
        if prompt.shown || window.has_active_dialog(cx) {
            return;
        }
        prompt.shown = true;
        let text: SharedString = match prompt.dirty {
            1 => "1 tab has unsaved edits. Closing it discards them.".into(),
            n => format!("{n} tabs have unsaved edits. Closing them discards the edits.").into(),
        };
        window.open_alert_dialog(cx, move |alert, _window, cx| {
            let discard = Button::new("close-tabs-confirm")
                .label("Discard and close")
                .primary()
                .bg(cx.theme().red)
                .border_color(cx.theme().red)
                .text_color(cx.theme().primary_foreground);
            alert
                .title("Discard unsaved edits?")
                .description(text.clone())
                .footer(
                    DialogFooter::new()
                        .child(
                            DialogClose::new().child(
                                Button::new("close-tabs-cancel")
                                    .outline()
                                    .label("Keep editing"),
                            ),
                        )
                        .child(DialogAction::new().child(discard)),
                )
                .on_ok(|_, window, cx| {
                    if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                        ws.update(cx, |ws, cx| ws.confirm_close_prompt(window, cx));
                    }
                    true
                })
                .on_cancel(|_, _, cx| {
                    if let Some(ws) = cx.try_global::<WorkspaceRef>().map(|w| w.0.clone()) {
                        ws.update(cx, |ws, cx| ws.cancel_close_prompt(cx));
                    }
                    true
                })
        });
    }

    /// Closable (unpinned) tabs the scope selects, in strip order.
    fn close_targets(
        &mut self,
        scope: CloseScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<EntityId> {
        let order: Vec<EntityId> = self.center_panels.iter().map(panel_entity_id).collect();
        let pos = |id: EntityId| order.iter().position(|p| *p == id);
        let active = active_live_center_panel(self.dock_area.read(cx), cx)
            .map(|p| panel_entity_id(&p))
            .or_else(|| {
                let tm = self.tab_manager.read(cx);
                tm.active_tab().map(|t| t.view.entity_id())
            });
        if scope == CloseScope::Active && active.is_none() {
            self.ensure_home_tab(window, cx);
            return Vec::new();
        }
        let tm = self.tab_manager.read(cx);
        order
            .iter()
            .copied()
            .filter(|&id| {
                let Some(tab) = tm.tab_for_panel_id(id) else {
                    return false;
                };
                !tab.pinned
                    && match scope {
                        CloseScope::Tab(t) => id == t,
                        CloseScope::Active => Some(id) == active,
                        CloseScope::Others(a) => id != a,
                        CloseScope::Left(a) => pos(a).zip(pos(id)).is_some_and(|(a, i)| i < a),
                        CloseScope::Right(a) => pos(a).zip(pos(id)).is_some_and(|(a, i)| i > a),
                        CloseScope::All => true,
                        CloseScope::Clean => !tab.dirty,
                        CloseScope::Project => {
                            tab.spec.conn_id().is_some_and(|c| !c.is_workspace_local())
                        }
                    }
            })
            .collect()
    }

    fn close_panels(&mut self, ids: &[EntityId], window: &mut Window, cx: &mut Context<Self>) {
        for &id in ids {
            let Some(panel) = self.find_center_panel(id, cx) else {
                continue;
            };
            if let Some(spec) = self
                .tab_manager
                .read(cx)
                .tab_for_panel_id(id)
                .map(|t| t.spec.clone())
            {
                self.forget_session_tab(&spec);
            }
            self.dock_area.update(cx, |dock, ecx| {
                remove_presentation_panel(dock, &panel, window, ecx);
            });
            self.unregister_center_panel(id);
        }
        self.sync_tab_manager_from_dock(cx);
        self.ensure_home_tab(window, cx);
        cx.notify();
    }
}
