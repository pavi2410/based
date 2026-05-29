//! Left sidebar mode: schema browser vs workspace query lane.

use gpui_component::IconName;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LeftPane {
    #[default]
    Browser,
    Workspace,
}

impl LeftPane {
    pub const ALL: [Self; 2] = [Self::Browser, Self::Workspace];

    pub fn icon(self) -> IconName {
        match self {
            Self::Browser => IconName::FolderOpen,
            Self::Workspace => IconName::BookOpen,
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Browser => "Connections & schema",
            Self::Workspace => "Loose queries & collections",
        }
    }
}
