//! Structured logging for the in-app updater (`RUST_LOG=based_updater=debug`).

pub const LOG_TARGET: &str = "based_updater";

pub fn info(msg: impl AsRef<str>) {
    log::info!(target: LOG_TARGET, "{}", msg.as_ref());
}

pub fn debug(msg: impl AsRef<str>) {
    log::debug!(target: LOG_TARGET, "{}", msg.as_ref());
}

pub fn warn(msg: impl AsRef<str>) {
    log::warn!(target: LOG_TARGET, "{}", msg.as_ref());
}
