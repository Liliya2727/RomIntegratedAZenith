//! Android properties, split by what it takes to write them.
//!
//! `getprop` / `setprop` go through the bionic calls in
//! [`crate::common::android_ffi`] instead of spawning their binaries — the
//! profile path is hot enough that a fork per property matters.
//!
//! `resetprop` still spawns, because only the Magisk/KSU tool can delete
//! properties and override `ro.*` ones.

use crate::common::android_ffi;
use std::fs;
use std::process::Command;

use crate::common::log::comp;
use crate::cmd::settings::utils::log_error;

pub(crate) fn getprop(key: &str) -> String {
    android_ffi::prop_get(key).unwrap_or_default()
}

pub(crate) fn resetprop(key: &str, val: &str) {
    let status = if val.is_empty() {
        Command::new("resetprop").arg("--delete").arg(key).status()
    } else {
        Command::new("resetprop").arg(key).arg(val).status()
    };

    if let Err(e) = status {
        log_error(comp::SETTINGS_PROP, &format!("resetprop failed key={} err={}", key, e));
    }
}

pub(crate) fn setprop(key: &str, val: &str) {
    if !android_ffi::prop_set(key, val) {
        log_error(comp::SETTINGS_PROP, &format!("setprop failed key={} value={}", key, val));
    }
}

/// Frequency offset ceiling in percent (100 = no limit). `Disabled`/empty and
/// anything unparsable fall back to no limit instead of failing the profile.
pub(crate) fn get_limiter() -> u64 {
    let val = getprop("persist.sys.azenithConfIntg.freqoffset");
    if val == "Disabled" || val.is_empty() {
        100
    } else {
        val.replace('%', "").parse().unwrap_or(100)
    }
}

pub(crate) fn get_clearapps() -> bool {
    getprop("persist.sys.azenithConfIntg.clearbg") == "1"
}

pub(crate) fn get_litemode() -> bool {
    getprop("persist.sys.azenithConfIntg.litemode") == "1"
}

pub(crate) fn get_curprofile() -> String {
    fs::read_to_string(crate::common::paths::CURRENT_PROFILE)
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Master switch: `persist.sys.azenithIntg.disabletweak = 1` disables every tweak.
pub(crate) fn is_tweak_disabled() -> bool {
    getprop("persist.sys.azenithIntg.disabletweak") == "1"
}
