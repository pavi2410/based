//! Shared query-editor chrome: history sidebar filters and variables popover.

use std::path::{Path, PathBuf};

use gpui::{
    Anchor, ElementId, FontWeight, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Sizable as _,
    button::{Button, ButtonVariants},
    h_flex,
    popover::Popover,
    v_flex,
};
use time::{Date, OffsetDateTime};

use crate::connection::ConnectionId;
use crate::query_store::{HistoryEntry, QueryStore};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HistoryFilter {
    #[default]
    All,
    Today,
}

impl HistoryFilter {
    pub const ALL: [Self; 2] = [Self::All, Self::Today];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Today => "Today",
        }
    }
}

pub fn open_vars_file(project_dir: &Path) {
    let path = project_dir.join(".based").join("vars.toml");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if !path.exists() {
        let _ = std::fs::write(&path, "[vars]\n");
    }
    let path_str = path.display().to_string();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&path_str).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open")
        .arg(&path_str)
        .spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", &path_str])
        .spawn();
}

pub fn filtered_history(
    store: &QueryStore,
    conn_id: &ConnectionId,
    filter: HistoryFilter,
) -> Vec<HistoryEntry> {
    let today = Date::from_calendar_date(
        OffsetDateTime::now_utc().year(),
        OffsetDateTime::now_utc().month(),
        OffsetDateTime::now_utc().day(),
    )
    .ok();

    store
        .history_for(conn_id)
        .into_iter()
        .filter(|e| match filter {
            HistoryFilter::All => true,
            HistoryFilter::Today => today.is_some_and(|d| {
                Date::from_calendar_date(e.ran_at.year(), e.ran_at.month(), e.ran_at.day()).ok()
                    == Some(d)
            }),
        })
        .take(50)
        .cloned()
        .collect()
}

/// Build a `Variables` toolbar trigger that opens a popover with the project's
/// `.based/vars.toml` map. The trigger button looks the same as before; clicking
/// it shows a panel listing the variables (mono font) and a shortcut to open
/// the vars file in the system editor.
pub fn variables_popover(
    id: impl Into<ElementId>,
    project_dir: Option<PathBuf>,
    vars: std::collections::HashMap<String, String>,
    mono_font: SharedString,
    cx: &gpui::App,
) -> Popover {
    let muted = cx.theme().muted_foreground;
    let trigger = Button::new("query-vars-trigger")
        .ghost()
        .small()
        .label("Variables");

    Popover::new(id)
        .anchor(Anchor::TopRight)
        .trigger(trigger)
        .content(move |_, _, _| {
            let project_dir = project_dir.clone();
            let mono = mono_font.clone();
            let vars = vars.clone();
            v_flex()
                .min_w(px(320.0))
                .gap(px(4.0))
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Variables (.based/vars.toml)"),
                        )
                        .when_some(project_dir.clone(), |row, root| {
                            row.child(
                                Button::new("vars-edit")
                                    .ghost()
                                    .xsmall()
                                    .label("Edit file")
                                    .on_click(move |_, _, _| open_vars_file(&root)),
                            )
                        }),
                )
                .children({
                    if vars.is_empty() {
                        vec![
                            div()
                                .py(px(4.0))
                                .text_xs()
                                .text_color(muted)
                                .child("No variables defined. Use $NAME in queries.")
                                .into_any_element(),
                        ]
                    } else {
                        vars.iter()
                            .map(|(k, v)| {
                                div()
                                    .py(px(2.0))
                                    .text_xs()
                                    .font_family(mono.clone())
                                    .child(format!("${k} = {v}"))
                                    .into_any_element()
                            })
                            .collect()
                    }
                })
        })
}
