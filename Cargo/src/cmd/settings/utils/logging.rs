//! Logging facade for the companion commands.
//!
//! The writer itself lives in [`crate::common::log`] — one implementation for
//! the daemon and every subcommand, so the on-disk shape stays identical:
//!
//! ```text
//! YYYY-MM-DD HH:MM:SS.mmm LEVEL [component] message
//! ```
//!
//! These wrappers only keep the call sites readable
//! (`log_info(comp::SETTINGS_CPU, "...")`) and the DEBUG gate in one place.
//! `component` is a dotted tag from [`crate::common::log::comp`].

use crate::common::log::{log_write, Lv};

/// A significant, low-frequency event (start/stop/profile change/config reload).
pub(crate) fn log_info(component: &str, message: &str) {
    log_write(Lv::Info, component, message);
}

/// DEBUG detail. Dropped unless `persist.sys.azenithIntg.debugmode` is on.
pub(crate) fn log_debug(component: &str, message: &str) {
    log_write(Lv::Debug, component, message);
}

/// An operation failed and needs attention. Include path/value/error context.
pub(crate) fn log_error(component: &str, message: &str) {
    log_write(Lv::Error, component, message);
}
