//! Forking `/system/bin/sh` for the handful of things that still need a
//! framework command (`vdc fstrim`, `cmd notification ...`, `am broadcast`).
//!
//! The daemon itself never comes through here — it only transacts.

use std::process::Command;

use crate::common::log::{log_write, Lv};

/// `PATH` for every forked shell (`vdc`, `sm`, `cmd`, `am`, and the toybox
/// helpers). Mirrors what Android's init exports, so nothing depends on a
/// module manager or a Termux install being present.
pub(crate) const PATH: &str = "/product/bin:/apex/com.android.runtime/bin:/apex/com.android.art/bin:/system_ext/bin:/system/bin:/system/xbin:/odm/bin:/vendor/bin:/vendor/xbin";

/// Run `command` and return its exit status (`-1` when it could not be run).
/// `component` is the caller's tag, so the line names the subsystem that
/// forked (`cmd.utility.shell`, ...).
pub(crate) fn system(component: &str, command: &str) -> i32 {
    match Command::new("/system/bin/sh").arg("-c").arg(command).env("PATH", PATH).status() {
        Ok(status) => status.code().unwrap_or(-1),
        Err(e) => {
            log_write(Lv::Error, component, &format!("system failed command={} err={}", command, e));
            -1
        }
    }
}

/// Run `command` and capture its trimmed stdout; `None` when it failed. The
/// failure is only a warning: callers use this for optional probes.
pub(crate) fn capture(component: &str, command: &str) -> Option<String> {
    match Command::new("/system/bin/sh").arg("-c").arg(command).env("PATH", PATH).output() {
        Ok(output) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        }
        Ok(output) => {
            log_write(Lv::Warn, component, &format!("command failed command={} status={}", command, output.status));
            None
        }
        Err(e) => {
            log_write(Lv::Error, component, &format!("capture failed command={} err={}", command, e));
            None
        }
    }
}
