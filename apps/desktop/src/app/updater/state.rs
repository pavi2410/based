use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use gpui_kit::SharedString;

/// Where the updater is. Each state carries only the data that is valid in it.
///
/// `B` is the downloaded bundle; generic so tests can build a `Ready` without a real update.
#[derive(Debug, Default)]
pub(super) enum Phase<B> {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
        /// The update can be downloaded and installed in-app (otherwise: releases page).
        can_install: bool,
    },
    Downloading {
        version: String,
        /// 0-100, written from the blocking download thread.
        progress: Arc<AtomicU8>,
    },
    /// Downloaded and verified, waiting for the user to approve the install.
    Ready {
        version: String,
        bundle: Box<B>,
    },
    Installing {
        version: String,
    },
    Failed {
        error: String,
    },
}

impl<B> Phase<B> {
    /// A new check may start: not while one runs, and never over a download in flight,
    /// a staged update, or an install.
    pub(super) fn allows_check(&self) -> bool {
        matches!(
            self,
            Phase::Idle | Phase::UpToDate | Phase::Available { .. } | Phase::Failed { .. }
        )
    }

    /// The newer version this state is about, if any.
    pub(super) fn version(&self) -> Option<&str> {
        match self {
            Phase::Available { version, .. }
            | Phase::Downloading { version, .. }
            | Phase::Ready { version, .. }
            | Phase::Installing { version } => Some(version),
            Phase::Idle | Phase::Checking | Phase::UpToDate | Phase::Failed { .. } => None,
        }
    }

    pub(super) fn status(&self) -> UpdateStatus {
        let label = |v: &str| SharedString::from(format!("v{}", v.trim_start_matches('v')));
        match self {
            Phase::Idle => UpdateStatus::Idle,
            Phase::Checking => UpdateStatus::Checking,
            Phase::UpToDate => UpdateStatus::UpToDate,
            Phase::Available {
                version,
                can_install,
            } => UpdateStatus::Available {
                version: label(version),
                can_install: *can_install,
            },
            Phase::Downloading { version, progress } => UpdateStatus::Downloading {
                version: label(version),
                percent: progress.load(Ordering::Relaxed),
            },
            Phase::Ready { version, .. } => UpdateStatus::Ready {
                version: label(version),
            },
            Phase::Installing { version } => UpdateStatus::Installing {
                version: label(version),
            },
            Phase::Failed { error } => UpdateStatus::Failed {
                error: error.clone().into(),
            },
        }
    }
}

/// Updater state for rendering (status bar, About). `version` is shown as `vX.Y.Z`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        version: SharedString,
        can_install: bool,
    },
    Downloading {
        version: SharedString,
        percent: u8,
    },
    Ready {
        version: SharedString,
    },
    Installing {
        version: SharedString,
    },
    Failed {
        error: SharedString,
    },
}

impl UpdateStatus {
    /// The newer version as `vX.Y.Z`, when the state is about one.
    pub fn version(&self) -> Option<&SharedString> {
        match self {
            UpdateStatus::Available { version, .. }
            | UpdateStatus::Downloading { version, .. }
            | UpdateStatus::Ready { version }
            | UpdateStatus::Installing { version } => Some(version),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type P = Phase<()>;

    fn all() -> Vec<P> {
        vec![
            P::Idle,
            P::Checking,
            P::UpToDate,
            P::Available {
                version: "1.2.0".into(),
                can_install: true,
            },
            P::Downloading {
                version: "1.2.0".into(),
                progress: Arc::new(AtomicU8::new(40)),
            },
            P::Ready {
                version: "1.2.0".into(),
                bundle: Box::new(()),
            },
            P::Installing {
                version: "1.2.0".into(),
            },
            P::Failed {
                error: "boom".into(),
            },
        ]
    }

    #[test]
    fn check_never_replaces_work_in_flight_or_a_staged_update() {
        let allowed: Vec<bool> = all().iter().map(Phase::allows_check).collect();
        assert_eq!(
            allowed,
            [true, false, true, true, false, false, false, true],
            "Idle, Checking, UpToDate, Available, Downloading, Ready, Installing, Failed"
        );
    }

    #[test]
    fn only_version_states_report_a_version() {
        let phases = all();
        let versions: Vec<Option<&str>> = phases.iter().map(Phase::version).collect();
        assert_eq!(
            versions,
            [
                None,
                None,
                None,
                Some("1.2.0"),
                Some("1.2.0"),
                Some("1.2.0"),
                Some("1.2.0"),
                None
            ]
        );
    }

    #[test]
    fn status_carries_each_states_data() {
        let phases = all();
        assert_eq!(
            phases[3].status(),
            UpdateStatus::Available {
                version: "v1.2.0".into(),
                can_install: true
            }
        );
        assert_eq!(
            phases[4].status(),
            UpdateStatus::Downloading {
                version: "v1.2.0".into(),
                percent: 40
            }
        );
        assert_eq!(
            phases[7].status(),
            UpdateStatus::Failed {
                error: "boom".into()
            }
        );
        assert_eq!(phases[0].status().version(), None);
    }

    #[test]
    fn status_label_does_not_double_the_v_prefix() {
        let p = P::Ready {
            version: "v2.0.0".into(),
            bundle: Box::new(()),
        };
        assert_eq!(p.status().version(), Some(&SharedString::from("v2.0.0")));
    }
}
