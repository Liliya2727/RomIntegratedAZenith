//
// Copyright (C) 2026-2027 Zexshia
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

//! Helpers for the `prefs` command (the former `binpreferenced` crate).
//!
//! Properties, node writes and the log writer all come from [`crate::common`];
//! this module only adds the pieces that are specific to the tweaks: the
//! read-once debug flag and `cmd_split`.

use std::process::{Command, Stdio};

use crate::common::log::{comp, log_write, Lv};
use crate::common::{fs, prop};

/// Snapshot the debug flag once at startup, so a tweak that runs for a while
/// cannot switch mid-run when the property flips.
pub(crate) fn init_debugmode() {
    DEBUGMODE.store(crate::common::log::debugmode(), std::sync::atomic::Ordering::Relaxed);
}

static DEBUGMODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn getprop(key: &str) -> String {
    prop::getprop(key)
}

pub(crate) fn resetprop(key: &str, val: &str) {
    // `resetprop -n` — the tweaks want the property changed without the
    // "persist" dance, so the `-n` form is spelled out here.
    let _ = Command::new("resetprop").arg("-n").arg(key).arg(val).status();
}

pub(crate) fn chmod_glob(pattern: &str, mode: u32) {
    // prefs-only: the thermal nodes are locked down for the whole run.
    fs::chmod_glob(pattern, mode);
}

pub(crate) fn write_val(value: &str, path_str: &str, lock: bool) -> bool {
    fs::write_val(value, path_str, &fs::node_name(path_str), lock, comp::PREFS_WRITE)
}
/// Run a command by argv, discarding its output: the tracing switches are
/// fire-and-forget.
pub(crate) fn cmd_split(parts: &[&str]) {
    let Some((program, args)) = parts.split_first() else { return };
    let _ = Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// DEBUG detail for the tweaks; only produced when debug mode is on.
pub(crate) fn log_verbose(component: &str, message: &str) {
    log_write(Lv::Debug, component, message);
}

/// A significant, low-frequency event.
pub(crate) fn log_info(component: &str, message: &str) {
    log_write(Lv::Info, component, message);
}
