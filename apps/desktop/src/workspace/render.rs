//! `impl Render for Workspace` — layout assembly and action wiring.
//!
//! Side-effect calls in `render` (session restore, queue flushing, home-tab guard) are
//! intentional: GPUI has no reliable post-init hook, so the first render frame is used as a
//! deferred executor.

use gpui::{Context, Entity, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme, Placement, v_flex};

use crate::bindings::{
    CloseAllTabs, CloseCleanTabs, CloseOtherTabs, CloseTab, CloseTabsLeft, CloseTabsRight,
    CycleAppearance, DismissCommandPalette, GoBackTab, GoForwardTab, NewQuery, OpenHome,
    OpenSettings, PinTab, SplitPaneBottom, SplitPaneLeft, SplitPaneRight, SplitPaneTop,
    ToggleCommandPalette, ToggleHistoryPane, ToggleInspectorPane, ToggleSavedPane,
    ToggleSidebarRail,
};
use crate::connection::{ConnectionEntry, ConnectionState};
use crate::project::ProjectContext;

use super::Workspace;
use super::chrome::{
    layout,
    left_pane::LeftPane,
    panes::{history_pane::render_history_pane, saved_pane::render_saved_pane},
    side_pane::{SidePane, render_side_pane},
    status_bar::{StatusBar, StatusBarModel},
    target_picker::render_target_picker,
    topbar::Topbar,
};
use super::panels::inspector::render_inspector_body;

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::app::quit::maybe_show_pending_close_dialog(self, window, cx);
        crate::project::open::maybe_show_pending_project_switch_dialog(self, window, cx);
        if crate::project::drain_pending_reload(cx)
            && let Some(pctx) = cx.try_global::<ProjectContext>()
        {
            self.project_title = pctx.project_name().into();
        }
        if !self.session_restored {
            self.session_restored = true;
            self.restore_session(window, cx);
        }
        self.flush_nav_queue(window, cx);
        self.flush_pending_open_tab(window, cx);
        self.ensure_home_tab(window, cx);
        let this = cx.entity().clone();
        let conn_list: Vec<Entity<ConnectionEntry>> = self.registry.read(cx).connections().to_vec();
        let conn_count = conn_list.len();
        let connected_count = conn_list
            .iter()
            .filter(|ent| matches!(ent.read(cx).state, ConnectionState::Connected(_)))
            .count();

        let selected_connection = self.connection_tree.read(cx).selected_connection_entry(cx);
        let focused_conn_id = self.focused_conn_id(cx);
        let active_pane = self.active_side_pane;
        let history_filter = self.history_filter;
        let workspace_for_panes = this.clone();
        let target_pick = self.pending_target_pick.clone();

        let side_pane: Option<gpui::AnyElement> = active_pane.map(|pane| {
            let body: gpui::AnyElement = match pane {
                SidePane::Inspector => {
                    render_inspector_body(selected_connection, window, cx).into_any_element()
                }
                SidePane::History => render_history_pane(
                    workspace_for_panes.clone(),
                    focused_conn_id.clone(),
                    history_filter,
                    cx,
                )
                .into_any_element(),
                SidePane::Saved => render_saved_pane(
                    focused_conn_id.clone(),
                    self.registry.clone(),
                    this.clone(),
                    cx,
                )
                .into_any_element(),
            };
            render_side_pane(pane, body, cx).into_any_element()
        });

        let sidebar = match self.active_left_pane {
            LeftPane::Browser => self.connection_tree.clone().into_any_element(),
            LeftPane::Workspace => v_flex()
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .child(crate::workspace::query_lane::render_query_lane(cx))
                .into_any_element(),
        };

        let body = layout::render_body_row(
            self.sidebar_collapsed,
            sidebar,
            self.dock_area.clone(),
            side_pane,
            cx,
        );

        let main = v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleSidebarRail, _, cx| {
                    ws.toggle_sidebar_rail(cx);
                }),
            )
            .on_action(window.listener_for(&this, |_, _: &CycleAppearance, _, cx| {
                crate::app::prefs::cycle_theme(cx);
            }))
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleCommandPalette, window, cx| {
                    ws.command_palette.update(cx, |p, cx| p.toggle(window, cx));
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &DismissCommandPalette, _, cx| {
                    ws.command_palette.update(cx, |p, cx| {
                        if p.is_visible() {
                            p.dismiss(cx);
                        }
                    });
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &OpenSettings, window, cx| {
                    ws.open_settings(window, cx);
                }),
            )
            .on_action(window.listener_for(&this, |ws, _: &OpenHome, window, cx| {
                ws.show_home(window, cx);
            }))
            .on_action(window.listener_for(&this, |ws, _: &CloseTab, window, cx| {
                ws.close_active_center_tab(window, cx);
            }))
            .on_action(window.listener_for(&this, |ws, _: &GoBackTab, window, cx| {
                ws.go_back_tab(window, cx);
            }))
            .on_action(
                window.listener_for(&this, |ws, _: &GoForwardTab, window, cx| {
                    ws.go_forward_tab(window, cx);
                }),
            )
            .on_action(window.listener_for(&this, |ws, _: &NewQuery, window, cx| {
                ws.open_new_query_tab(window, cx);
            }))
            .on_action(
                window.listener_for(&this, |ws, _: &CloseAllTabs, window, cx| {
                    ws.close_all_tabs(window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &CloseCleanTabs, window, cx| {
                    ws.close_clean_tabs(window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &CloseOtherTabs, window, cx| {
                    if let Some(id) = ws.active_center_panel_id(cx) {
                        ws.close_other_tabs(id, window, cx);
                    }
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &CloseTabsLeft, window, cx| {
                    if let Some(id) = ws.active_center_panel_id(cx) {
                        ws.close_tabs_to_left(id, window, cx);
                    }
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &CloseTabsRight, window, cx| {
                    if let Some(id) = ws.active_center_panel_id(cx) {
                        ws.close_tabs_to_right(id, window, cx);
                    }
                }),
            )
            .on_action(window.listener_for(&this, |ws, _: &PinTab, _, cx| {
                if let Some(id) = ws.active_center_panel_id(cx) {
                    ws.toggle_pin_tab(id, cx);
                }
            }))
            .on_action(
                window.listener_for(&this, |ws, _: &SplitPaneTop, window, cx| {
                    ws.split_center_pane(Placement::Top, window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &SplitPaneBottom, window, cx| {
                    ws.split_center_pane(Placement::Bottom, window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &SplitPaneLeft, window, cx| {
                    ws.split_center_pane(Placement::Left, window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &SplitPaneRight, window, cx| {
                    ws.split_center_pane(Placement::Right, window, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleInspectorPane, _, cx| {
                    ws.toggle_side_pane(SidePane::Inspector, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleHistoryPane, _, cx| {
                    ws.toggle_side_pane(SidePane::History, cx);
                }),
            )
            .on_action(
                window.listener_for(&this, |ws, _: &ToggleSavedPane, _, cx| {
                    ws.toggle_side_pane(SidePane::Saved, cx);
                }),
            )
            .bg(cx.theme().background)
            .child(Topbar::new(self.registry.clone()))
            .child(body)
            .child(StatusBar::new(
                StatusBarModel {
                    connection_count: conn_count,
                    connected_count,
                    scope_label: self.project_title.clone(),
                    history_ready: !cx
                        .global::<crate::query_store::QueryStore>()
                        .history
                        .recent(1)
                        .is_empty(),
                    update: crate::app::updater::coordinator_snapshot(cx),
                },
                active_pane,
                self.active_left_pane,
                self.registry.clone(),
            ))
            .child(self.command_palette.clone());

        let stacked = super::chrome::overlays::stack_gpui_overlays(main, window, cx);
        div()
            .size_full()
            .relative()
            .child(stacked)
            .when_some(target_pick, |layer, pick| {
                layer.child(render_target_picker(
                    &pick.0,
                    &pick.1,
                    &self.registry,
                    this.clone(),
                    cx,
                ))
            })
    }
}
