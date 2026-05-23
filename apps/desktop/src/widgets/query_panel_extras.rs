//! Shared query-editor chrome: history sidebar filters, star-to-save, variables footer.

use std::collections::HashSet;
use std::path::PathBuf;

use gpui::{FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, prelude::*};
use gpui_component::{
    Sizable as _,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use time::{Date, OffsetDateTime};

use crate::connection::ConnectionId;
use crate::query_store::{HistoryEntry, QueryStore, SavedQuery};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HistoryFilter {
    #[default]
    All,
    Saved,
    Today,
}

impl HistoryFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Saved, Self::Today];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Saved => "Saved ★",
            Self::Today => "Today",
        }
    }
}

pub fn open_vars_file(project_dir: &PathBuf) {
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
    let _ = std::process::Command::new("xdg-open").arg(&path_str).spawn();
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
    let saved_texts: HashSet<String> = store
        .saved
        .for_conn(conn_id)
        .into_iter()
        .map(|q| q.query_text().to_string())
        .collect();
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
            HistoryFilter::Saved => saved_texts.contains(&e.query),
            HistoryFilter::Today => today.is_some_and(|d| {
                Date::from_calendar_date(e.ran_at.year(), e.ran_at.month(), e.ran_at.day()).ok()
                    == Some(d)
            }),
        })
        .cloned()
        .take(50)
        .collect()
}

pub fn save_starred_query(
    store: &mut QueryStore,
    conn_id: ConnectionId,
    name: &str,
    query: &str,
    is_mongo: bool,
    mongo_collection: Option<String>,
) {
    let id = format!(
        "q_{}",
        name.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
    );
    let mut q = SavedQuery {
        id,
        name: name.to_string(),
        connection: conn_id,
        tags: vec![],
        sql: None,
        pipeline: None,
        mongo_collection,
    };
    if is_mongo {
        q.pipeline = Some(query.to_string());
    } else {
        q.sql = Some(query.to_string());
    }
    store.save_query(q);
}

pub fn variables_footer(
    project_dir: Option<PathBuf>,
    show: bool,
    vars: std::collections::HashMap<String, String>,
    mono_font: SharedString,
    border: Hsla,
    muted: Hsla,
    muted_bg: Hsla,
) -> impl IntoElement {
    v_flex()
        .when(show, |col| {
            col.border_t_1()
                .border_color(border)
                .bg(muted_bg)
                .child(
                    h_flex()
                        .px_3()
                        .py_2()
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
                        vec![div()
                            .px_3()
                            .pb_2()
                            .text_xs()
                            .text_color(muted)
                            .child("No variables defined. Use $NAME in queries.")
                            .into_any_element()]
                    } else {
                        vars.iter()
                            .map(|(k, v)| {
                                div()
                                    .px_3()
                                    .py_1()
                                    .text_xs()
                                    .font_family(mono_font.clone())
                                    .child(format!("${k} = {v}"))
                                    .into_any_element()
                            })
                            .collect()
                    }
                })
        })
}
