//! Native folder picker for opening a `.based/` project.

use std::path::PathBuf;

use gpui::AsyncApp;

use crate::db;

/// Show a blocking folder picker off the UI thread.
pub async fn pick_project_folder(cx: &mut AsyncApp) -> Option<PathBuf> {
    db::run_infallible(cx, async {
        tokio::task::spawn_blocking(|| {
            rfd::FileDialog::new()
                .set_title("Open Project")
                .pick_folder()
        })
        .await
        .ok()
        .flatten()
    })
    .await
    .ok()
    .flatten()
}
