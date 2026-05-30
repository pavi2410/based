use gpui::SharedString;

/// High-level updater UI / coordinator phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UpdatePhase {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Ready,
    Failed,
}

impl UpdatePhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "",
            Self::Checking => "Checking…",
            Self::UpToDate => "Up to date",
            Self::Available => "Update available",
            Self::Downloading => "Downloading…",
            Self::Ready => "Restart to update",
            Self::Failed => "Update failed",
        }
    }
}

/// Snapshot for status bar rendering.
#[derive(Clone, Debug, Default)]
pub struct UpdateBarSnapshot {
    pub phase: UpdatePhase,
    pub version: Option<SharedString>,
    pub progress_percent: u8,
    pub error: Option<SharedString>,
}
