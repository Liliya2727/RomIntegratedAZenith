//! Logging wrappers for the `utility` command.
//!
//! The writer lives in [`crate::common::log`]; these four functions only keep
//! the call sites (and the DEBUG gate) readable.

use crate::common::log::{log_write, Lv};

/// A significant, low-frequency event.
pub(crate) fn log_info(component: &str, message: &str) {
    log_write(Lv::Info, component, message);
}

/// DEBUG detail. Dropped unless debug mode is on; routed to the verbose log.
pub(crate) fn log_debug(component: &str, message: &str) {
    log_write(Lv::Debug, component, message);
}

/// An oddity the caller recovered from; execution continues.
pub(crate) fn log_warn(component: &str, message: &str) {
    log_write(Lv::Warn, component, message);
}

/// An operation failed and needs attention. Include path/value/error context.
pub(crate) fn log_error(component: &str, message: &str) {
    log_write(Lv::Error, component, message);
}
