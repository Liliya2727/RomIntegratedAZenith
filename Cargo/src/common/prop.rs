//! System property access, shared by the daemon and the companion commands.

use std::process::Command;

use crate::common::log::{comp, Lv, log_write};

// Path semua file di Config.toml (root repo) — jangan tambah path hardcoded di sini.

/// Returns the property value, or `""` when unset/empty.
pub(crate) fn getprop(key: &str) -> String {
    match Command::new("getprop").arg(key).output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(e) => {
            log_write(Lv::Error, comp::COMMON_PROP, &format!("getprop failed key={} err={}", key, e));
            String::new()
        }
    }
}

/// Set a property. An empty `val` deletes it, which only the Magisk/KSU
/// `resetprop` tool can do (it also overrides `ro.*` values).
pub(crate) fn resetprop(key: &str, val: &str) {
    let status = if val.is_empty() {
        Command::new("resetprop").arg("--delete").arg(key).status()
    } else {
        Command::new("resetprop").arg(key).arg(val).status()
    };

    if let Err(e) = status {
        log_write(Lv::Error, comp::COMMON_PROP, &format!("resetprop failed key={} err={}", key, e));
    }
}

pub(crate) fn setprop(key: &str, val: &str) {
    if let Err(e) = Command::new("setprop").arg(key).arg(val).status() {
        log_write(Lv::Error, comp::COMMON_PROP, &format!("setprop failed key={} value={} err={}", key, val, e));
    }
}
