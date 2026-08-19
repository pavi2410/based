//! Where a new connection is saved: the open project or the personal based-dir.

use std::path::{Path, PathBuf};

use gpui::{Context, Hsla, IntoElement, ParentElement, prelude::*};
use gpui_component::{
    button::{Button, ButtonVariants},
    h_flex,
};

use crate::connection::ConnectionOrigin;
use crate::project::personal::personal_root;
use crate::widgets::labeled_field;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionDestination {
    Project,
    Personal,
}

impl ConnectionDestination {
    pub fn origin(self) -> ConnectionOrigin {
        match self {
            Self::Project => ConnectionOrigin::Project,
            Self::Personal => ConnectionOrigin::Personal,
        }
    }

    pub fn from_origin(origin: ConnectionOrigin) -> Self {
        match origin {
            ConnectionOrigin::Project => Self::Project,
            ConnectionOrigin::Personal => Self::Personal,
        }
    }

    pub fn based_dir(self, project_dir: Option<&Path>) -> anyhow::Result<PathBuf> {
        match self {
            Self::Personal => Ok(personal_root()),
            Self::Project => project_dir
                .map(|p| p.join(".based"))
                .ok_or_else(|| anyhow::anyhow!("no project is open")),
        }
    }
}

pub fn resolve_wizard_destination(
    has_project: bool,
    chosen: Option<ConnectionDestination>,
) -> Result<ConnectionDestination, String> {
    if has_project {
        chosen.ok_or_else(|| "Choose This project or Personal.".into())
    } else {
        Ok(ConnectionDestination::Personal)
    }
}

pub fn destination_row<T: 'static>(
    selected: Option<ConnectionDestination>,
    muted: Hsla,
    cx: &mut Context<T>,
    set: fn(&mut T, ConnectionDestination),
) -> impl IntoElement {
    labeled_field(
        "Save to",
        muted,
        h_flex()
            .gap_2()
            .child(
                Button::new("dest-project")
                    .label("This project")
                    .when(selected == Some(ConnectionDestination::Project), |b| {
                        b.primary()
                    })
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        set(panel, ConnectionDestination::Project);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("dest-personal")
                    .label("Personal")
                    .when(selected == Some(ConnectionDestination::Personal), |b| {
                        b.primary()
                    })
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        set(panel, ConnectionDestination::Personal);
                        cx.notify();
                    })),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_project_always_saves_personal() {
        assert_eq!(
            resolve_wizard_destination(false, None).unwrap(),
            ConnectionDestination::Personal
        );
        assert_eq!(
            resolve_wizard_destination(false, Some(ConnectionDestination::Project)).unwrap(),
            ConnectionDestination::Personal
        );
    }

    #[test]
    fn project_open_requires_an_explicit_choice() {
        assert!(resolve_wizard_destination(true, None).is_err());
        assert_eq!(
            resolve_wizard_destination(true, Some(ConnectionDestination::Personal)).unwrap(),
            ConnectionDestination::Personal
        );
        assert_eq!(
            resolve_wizard_destination(true, Some(ConnectionDestination::Project)).unwrap(),
            ConnectionDestination::Project
        );
    }
}
