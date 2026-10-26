//! One status line for panels: idle, busy, done, warning or error.

use gpui_kit::component::ActiveTheme;
use gpui_kit::{AnyElement, App, SharedString, div, prelude::*};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PanelStatus {
    #[default]
    Idle,
    Busy(SharedString),
    Done(SharedString),
    Warn(SharedString),
    Error(SharedString),
}

impl PanelStatus {
    pub fn busy(msg: impl Into<SharedString>) -> Self {
        Self::Busy(msg.into())
    }

    pub fn done(msg: impl Into<SharedString>) -> Self {
        Self::Done(msg.into())
    }

    pub fn warn(msg: impl Into<SharedString>) -> Self {
        Self::Warn(msg.into())
    }

    pub fn error(msg: impl Into<SharedString>) -> Self {
        Self::Error(msg.into())
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Status line colored by severity; `None` when idle.
    pub fn line(&self, cx: &App) -> Option<AnyElement> {
        let theme = cx.theme();
        let (msg, color) = match self {
            Self::Idle => return None,
            Self::Busy(m) | Self::Done(m) => (m, theme.muted_foreground),
            Self::Warn(m) => (m, theme.warning_foreground),
            Self::Error(m) => (m, theme.danger),
        };
        Some(
            div()
                .px_3()
                .py_1()
                .text_xs()
                .text_color(color)
                .child(msg.clone())
                .into_any_element(),
        )
    }
}
