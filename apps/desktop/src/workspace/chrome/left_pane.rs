//! Left sidebar mode: schema browser vs workspace query lane.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LeftPane {
    #[default]
    Browser,
    Workspace,
}
