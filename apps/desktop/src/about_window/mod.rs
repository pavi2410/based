//! Single-instanced "About Based" window.
//!
//! Surfaced from the macOS app menu and the topbar overflow menu. Tracked by
//! [`crate::app::aux_windows::AuxWindows`] so re-opening focuses the existing
//! window; closes when the main workspace window closes (see
//! [`crate::workspace::pop_out::PopOutManager::on_any_window_closed`]).

use gpui::{
    ClipboardItem, Context, FocusHandle, Focusable, FontWeight, Hsla, IntoElement, MouseButton,
    ParentElement, Render, SharedString, Styled, Window, div, img, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

const APP_NAME: &str = "Based";
const TAGLINE: &str = "Git-Friendly Database Client";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMIT: &str = env!("BASED_GIT_SHA");
const LICENSE: &str = env!("CARGO_PKG_LICENSE");

const WEBSITE_URL: &str = "https://based.pavi2410.com";
const REPO_URL: &str = "https://github.com/pavi2410/based";
const DISCUSSIONS_URL: &str = "https://github.com/pavi2410/based/discussions";
const ISSUES_URL: &str = "https://github.com/pavi2410/based/issues/new/choose";
const RELEASES_URL: &str = "https://github.com/pavi2410/based/releases";
const LICENSE_URL: &str = "https://github.com/pavi2410/based/blob/HEAD/LICENSE";
const DEV_SITE_URL: &str = "https://pavi2410.com";
const DEV_GITHUB_URL: &str = "https://github.com/pavi2410";
const SPONSORS_URL: &str = "https://github.com/sponsors/pavi2410";

/// Short curated list. Kept in sync manually with the workspace `Cargo.toml`;
/// transitive dep changes don't need to be reflected here.
const TECH_STACK: &[&str] = &[
    "Rust",
    "GPUI",
    "gpui-component",
    "tokio",
    "sqlx",
    "mongodb",
    "serde",
    "toml",
];

pub struct AboutWindow {
    focus_handle: FocusHandle,
}

impl AboutWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for AboutWindow {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AboutWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let bg = theme.background;
        let fg = theme.foreground;
        let muted = theme.muted_foreground;
        let border = theme.border;
        let link = theme.accent_foreground;
        let hover_bg = theme.muted.opacity(0.35);

        v_flex()
            .id("about-window")
            .size_full()
            .bg(bg)
            .text_color(fg)
            .pt(px(24.0))
            .px(px(24.0))
            .pb(px(16.0))
            .gap(px(12.0))
            .child(hero(fg, muted))
            .child(update_card(muted, border, link, cx))
            .child(links_columns(muted, link, hover_bg))
            .child(footer(border, muted, link, hover_bg))
    }
}

fn hero(fg: Hsla, muted: Hsla) -> impl IntoElement {
    v_flex()
        .items_center()
        .gap(px(6.0))
        .child(img("icon.png").size(px(56.0)).rounded(px(12.0)).shadow_sm())
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(FontWeight::BOLD)
                .text_color(fg)
                .child(APP_NAME),
        )
        .child(
            div()
                .text_size(px(12.0))
                .italic()
                .text_color(muted)
                .child(TAGLINE),
        )
        .child(version_row(muted))
}

fn version_row(muted: Hsla) -> impl IntoElement {
    let copy_text: SharedString = format!("Based v{VERSION} ({COMMIT})").into();
    let display: SharedString = format!("v{VERSION}  ·  commit {COMMIT}").into();

    h_flex()
        .items_center()
        .justify_center()
        .gap(px(4.0))
        .child(div().text_size(px(11.0)).text_color(muted).child(display))
        .child(
            Button::new("copy-version")
                .ghost()
                .xsmall()
                .icon(IconName::Copy)
                .tooltip(SharedString::from("Copy version and commit"))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_text.to_string()));
                }),
        )
}

fn update_card(muted: Hsla, border: Hsla, accent: Hsla, cx: &gpui::App) -> impl IntoElement {
    let checks_locked = crate::app::prefs::update_check_settings_locked();
    let snapshot = crate::app::updater::coordinator_snapshot(cx);
    let status: SharedString = if checks_locked {
        "Dev build — update checks disabled".into()
    } else {
        match snapshot.phase {
            crate::app::updater::UpdatePhase::Idle => "Check for updates to see status".into(),
            crate::app::updater::UpdatePhase::Checking => "Checking for updates…".into(),
            crate::app::updater::UpdatePhase::UpToDate => "You're up to date".into(),
            crate::app::updater::UpdatePhase::Available => snapshot
                .version
                .map(|v| format!("{v} is available").into())
                .unwrap_or_else(|| "Update available".into()),
            crate::app::updater::UpdatePhase::Downloading => "Downloading update…".into(),
            crate::app::updater::UpdatePhase::Ready => "Update ready — restart to apply".into(),
            crate::app::updater::UpdatePhase::Failed => snapshot
                .error
                .unwrap_or_else(|| "Update check failed".into()),
        }
    };

    v_flex()
        .p(px(12.0))
        .gap(px(8.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(border)
        .bg(muted.opacity(0.06))
        .child(div().text_size(px(11.0)).text_color(muted).child(status))
        .child(
            h_flex()
                .gap(px(8.0))
                .when(crate::app::prefs::manual_update_checks_enabled(), |row| {
                    row.child(
                        Button::new("about-check-updates")
                            .outline()
                            .small()
                            .label("Check for Updates")
                            .on_click(|_, _, cx| crate::app::updater::check_now(cx)),
                    )
                })
                .child(
                    Button::new("about-release-notes")
                        .ghost()
                        .small()
                        .label("Release Notes")
                        .text_color(accent)
                        .on_click(|_, _, cx| {
                            crate::app::updater::open_release_notes_for_current(cx);
                        }),
                ),
        )
}

fn links_columns(muted: Hsla, link: Hsla, hover_bg: Hsla) -> impl IntoElement {
    h_flex()
        .gap(px(20.0))
        .child(
            v_flex()
                .flex_1()
                .gap(px(2.0))
                .child(section_eyebrow("Links", muted))
                .child(link_row(
                    "Website",
                    WEBSITE_URL,
                    IconName::Globe,
                    link,
                    hover_bg,
                    false,
                ))
                .child(link_row(
                    "Repository",
                    REPO_URL,
                    IconName::Github,
                    link,
                    hover_bg,
                    false,
                ))
                .child(link_row(
                    "Discussions",
                    DISCUSSIONS_URL,
                    IconName::Globe,
                    link,
                    hover_bg,
                    false,
                ))
                .child(link_row(
                    "Report an issue",
                    ISSUES_URL,
                    IconName::Info,
                    link,
                    hover_bg,
                    false,
                ))
                .child(link_row(
                    "Releases",
                    RELEASES_URL,
                    IconName::BookOpen,
                    link,
                    hover_bg,
                    false,
                )),
        )
        .child(
            v_flex()
                .flex_1()
                .gap(px(2.0))
                .child(section_eyebrow("Developer", muted))
                .child(
                    h_flex()
                        .h(px(28.0))
                        .px(px(6.0))
                        .items_center()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("pavi2410"),
                )
                .child(link_row(
                    "pavi2410.com",
                    DEV_SITE_URL,
                    IconName::Globe,
                    link,
                    hover_bg,
                    false,
                ))
                .child(link_row(
                    "GitHub profile",
                    DEV_GITHUB_URL,
                    IconName::Github,
                    link,
                    hover_bg,
                    false,
                ))
                .child(link_row(
                    "Sponsor on GitHub",
                    SPONSORS_URL,
                    IconName::Heart,
                    link,
                    hover_bg,
                    true,
                )),
        )
}

fn footer(border: Hsla, muted: Hsla, link: Hsla, hover_bg: Hsla) -> impl IntoElement {
    let license_label = format!("{LICENSE} — view LICENSE");
    v_flex()
        .gap(px(8.0))
        .pt(px(4.0))
        .child(tech_stack_row(border, muted))
        .child(link_row(
            &license_label,
            LICENSE_URL,
            IconName::ExternalLink,
            link,
            hover_bg,
            false,
        ))
}

fn section_eyebrow(label: &str, muted: Hsla) -> impl IntoElement {
    div()
        .pb(px(4.0))
        .text_size(px(10.0))
        .font_weight(FontWeight::BOLD)
        .text_color(muted)
        .child(label.to_uppercase())
}

/// Inline link with icon, hover highlight, and system browser as click target.
fn link_row(
    label: &str,
    url: &'static str,
    icon: IconName,
    color: Hsla,
    hover_bg: Hsla,
    emphasis: bool,
) -> impl IntoElement {
    let label_owned: SharedString = label.to_string().into();
    h_flex()
        .id(SharedString::from(format!("link-{url}")))
        .h(px(28.0))
        .px(px(6.0))
        .rounded(px(6.0))
        .items_center()
        .gap(px(8.0))
        .text_size(px(12.0))
        .text_color(color)
        .when(emphasis, |row| row.font_weight(FontWeight::SEMIBOLD))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .child(Icon::new(icon).xsmall())
        .child(div().child(label_owned))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.open_url(url);
        })
}

fn tech_stack_row(border: Hsla, muted: Hsla) -> impl IntoElement {
    let mut row = h_flex().flex_wrap().gap(px(5.0));
    for name in TECH_STACK {
        row = row.child(
            div()
                .px(px(7.0))
                .py(px(2.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(border)
                .text_size(px(10.0))
                .text_color(muted)
                .child(name.to_string()),
        );
    }
    row
}
