//! Read-only diff of an AI-proposed edit against the current text.

use based_ai::patch;
use gpui_kit::component::diff::{Diff, DiffFile, DiffState};
use gpui_kit::{Entity, IntoElement, Pixels, Styled, div, prelude::*, px};

/// Approximate row height and the hunk header plus surrounding context rows.
const ROW_PX: f32 = 22.0;
const CONTEXT_ROWS: usize = 7;

pub struct AiDiff {
    state: Entity<DiffState>,
    additions: usize,
    deletions: usize,
}

impl AiDiff {
    /// `None` when the proposal equals `old` or the patch cannot be parsed.
    pub fn new(
        path: &str,
        language: &str,
        old: &str,
        new: &str,
        cx: &mut gpui_kit::App,
    ) -> Option<Self> {
        let files = parse(path, language, old, new)?;
        let additions = files.iter().map(DiffFile::additions).sum();
        let deletions = files.iter().map(DiffFile::deletions).sum();
        let state = cx.new(|cx| DiffState::new(files, cx));
        Some(Self {
            state,
            additions,
            deletions,
        })
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.additions, self.deletions)
    }

    /// The diff surface. It is a virtualized list that fills its parent, so it needs a
    /// definite height (a max height collapses it): sized to the change, capped at `max_height`.
    pub fn view(&self, max_height: Pixels) -> impl IntoElement {
        let rows = (self.additions + self.deletions + CONTEXT_ROWS) as f32;
        div()
            .w_full()
            .h((px(rows * ROW_PX)).min(max_height))
            .child(Diff::new(&self.state).header_visible(false))
    }
}

fn parse(path: &str, language: &str, old: &str, new: &str) -> Option<Vec<DiffFile>> {
    let text = patch::unified(path, old, new)?;
    let files = DiffFile::parse(&text).ok()?;
    Some(
        files
            .into_iter()
            .map(|f| f.with_language(language.to_owned()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generated_patch() {
        let files = parse(
            "query.sql",
            "sql",
            "SELECT a\nFROM t\n",
            "SELECT b\nFROM t\nLIMIT 5\n",
        )
        .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!((files[0].additions(), files[0].deletions()), (2, 1));
        assert_eq!(files[0].language().as_ref(), "sql");
    }

    #[test]
    fn new_text_into_empty_editor() {
        let files = parse("query.sql", "sql", "", "SELECT 1\n").unwrap();
        assert_eq!((files[0].additions(), files[0].deletions()), (1, 0));
    }

    #[test]
    fn no_change_is_none() {
        assert!(parse("query.sql", "sql", "SELECT 1\n", "SELECT 1\n").is_none());
    }

    #[test]
    fn missing_trailing_newline_still_parses() {
        assert!(parse("query.sql", "sql", "SELECT 1", "SELECT 2").is_some());
    }
}
