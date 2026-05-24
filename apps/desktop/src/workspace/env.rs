//! Workspace environment selector shown in the title bar.
//!
//! Today only `local` is available; extend [`ENV_OPTIONS`] when env switching ships.

/// Selectable workspace environments (display label = stored value).
pub const ENV_OPTIONS: &[&'static str] = &["local"];
