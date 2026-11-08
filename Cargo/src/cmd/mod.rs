//! The companion tools, all linked into this one binary.
//!
//! Four standalone executables used to ship next to the daemon; their code now
//! lives in modules here:
//!
//! | Removed binary | Was | Module |
//! |---|---|---|
//! | `sys.azenith-profilesettings` | crate `binprofiles` | [`settings`] |
//! | `sys.azenith-utilityconf` | crate `binutils` | [`utility`] |
//! | `sys.azenith-preferencedtweaks` | crate `binpreferenced` | [`prefs`] |
//! | `sys.azenith-preloadbin` | C `preloadbin` (vmtouch) | [`preload`] |
//!
//! The daemon calls these modules directly (`settings::apply_mode`,
//! `utility::trim`, `prefs::apply`) instead of re-exec'ing itself. The CLI
//! names remain as thin wrappers for manual/debug use, dispatched by
//! [`main`](crate::main).
//!
//! Cross-cutting helpers (log writer, properties, sysfs writes, forked shell)
//! live in [`crate::common`], not here.

pub(crate) mod prefs;
pub(crate) mod preload;
pub(crate) mod settings;
pub(crate) mod utility;

use std::fs;

pub(crate) const PROFILESETTINGS: &str = "profilesettings";
pub(crate) const UTILITYCONF: &str = "utilityconf";
pub(crate) const PREFERENCEDTWEAKS: &str = "preferencedtweaks";

fn parent_pid() -> Option<u32> {
    fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|stat| stat.split_whitespace().nth(3)?.parse::<u32>().ok())
}

fn process_cmdline(pid: u32) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/cmdline"))
        .ok()
        .map(|s| s.replace('\0', " ").trim().to_string())
}

/// `true` when the parent process is the service. The CLI entry points keep
/// this guard so a human invoking `profilesettings` / `preferencedtweaks` from
/// a shell is rejected; the daemon's internal paths call the dedicated
/// `apply_mode` / `apply` functions and skip it.
pub(crate) fn verify_caller() -> bool {
    let Some(cmdline) = parent_pid().and_then(process_cmdline) else { return false };
    // `azenith-service` covers both the installed name
    // (`/vendor/bin/hw/vendor.azenith-service`, what init runs) and the
    // `sys.azenith-service` name used by service scripts; the legacy
    // `sys.azenith*` binary names are still accepted for older setups.
    cmdline.contains("azenith-service") || cmdline.contains("sys.azenith")
}
