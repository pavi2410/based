//! Where a panel's background load stands, as a single value.
//!
//! One enum instead of `loading` / `error` flags beside the data, so states such as
//! "loading and failed at once" cannot be written down. The loaded data itself lives
//! where the panel shows it (a grid, an editor), so `Loaded` carries none.

use gpui_kit::SharedString;

/// Why a load failed, for the panel's error card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadFailure {
    pub title: &'static str,
    /// The first line of the underlying error.
    pub detail: SharedString,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Load {
    /// Nothing requested yet.
    #[default]
    Idle,
    Loading,
    Loaded,
    Failed(LoadFailure),
}

impl LoadFailure {
    /// A failure titled `title`, keeping only the first line of `detail`.
    pub fn new(title: &'static str, detail: impl ToString) -> Self {
        let detail = detail.to_string();
        let first = detail.lines().next().unwrap_or("Unknown error").to_owned();
        Self {
            title,
            detail: first.into(),
        }
    }
}

impl Load {
    /// A failure titled `title`, keeping only the first line of `detail`.
    pub fn failed(title: &'static str, detail: impl ToString) -> Self {
        Self::Failed(LoadFailure::new(title, detail))
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub fn failure(&self) -> Option<&LoadFailure> {
        match self {
            Self::Failed(f) => Some(f),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_keeps_the_first_line() {
        let load = Load::failed("Couldn't load", "boom\nstack trace");
        assert_eq!(load.failure().map(|f| f.detail.as_ref()), Some("boom"));
        assert!(!load.is_loading());
    }
}
