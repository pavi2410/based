//! Center-tab panel showing GitHub release notes markdown for a version.

use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, px,
};
use gpui_component::{
    ActiveTheme,
    dock::{Panel, PanelEvent},
    scroll::ScrollableElement,
    v_flex,
};

pub struct ReleaseNotesPanel {
    focus_handle: FocusHandle,
    pub(crate) tab_label: SharedString,
    version: String,
    body: Option<String>,
    error: Option<String>,
    loading: bool,
}

impl ReleaseNotesPanel {
    pub fn version_label(&self, _: &App) -> String {
        self.version.clone()
    }

    pub fn new(version: String, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let label: SharedString = format!("What's New in v{version}").into();
        let version_fetch = version.clone();
        cx.spawn(async move |this, cx| {
            let result = crate::db::run(cx, async move {
                crate::app::updater::fetch_release_body(&version_fetch).await
            })
            .await;
            let _ = this.update(cx, |panel, cx| {
                panel.loading = false;
                match result {
                    Ok(body) => panel.body = Some(body),
                    Err(err) => panel.error = Some(format!("{err:#}")),
                }
                cx.notify();
            });
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            tab_label: label,
            version,
            body: None,
            error: None,
            loading: true,
        }
    }
}

impl gpui::EventEmitter<PanelEvent> for ReleaseNotesPanel {}

impl Focusable for ReleaseNotesPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for ReleaseNotesPanel {
    fn panel_name(&self) -> &'static str {
        "ReleaseNotesPanel"
    }

    fn dropdown_menu(
        &mut self,
        menu: gpui_component::menu::PopupMenu,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_component::menu::PopupMenu {
        crate::based_panel_dropdown!(menu, self, cx)
    }

    crate::based_panel_tab_chrome!();
}

impl Render for ReleaseNotesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;

        let content = if self.loading {
            div()
                .text_size(px(13.0))
                .text_color(muted)
                .child("Loading release notes…")
        } else if let Some(err) = &self.error {
            div()
                .text_size(px(13.0))
                .text_color(cx.theme().danger_foreground)
                .child(err.clone())
        } else {
            div()
                .text_size(px(13.0))
                .text_color(fg)
                .line_height(px(20.0))
                .child(self.body.clone().unwrap_or_default())
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(
                div().px(px(24.0)).pt(px(20.0)).pb(px(8.0)).child(
                    div()
                        .text_size(px(18.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("What's New in v{}", self.version)),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px(px(24.0))
                    .pb(px(24.0))
                    .overflow_y_scrollbar()
                    .child(content),
            )
    }
}
