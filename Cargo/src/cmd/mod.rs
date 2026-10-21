//! Who the companion command is, and how it is reached.
//!
//! Four standalone executables used to ship next to the daemon; they are now
//! subcommands of this binary:
//!
//! | Removed binary | Was | Now |
//! |---|---|---|
//! | `sys.azenith-profilesettings` | crate `binprofiles` | `sys.azenith-service profilesettings` |
//! | `sys.azenith-utilityconf` | crate `binutils` | `sys.azenith-service utilityconf` |
//! | `sys.azenith-preferencedtweaks` | crate `binpreferenced` | `sys.azenith-service preferencedtweaks` |
//! | `sys.azenith-preloadbin` | C `preloadbin` (vmtouch) | `sys.azenith-service preload` |
//!
//! [`main`](crate::main) picks the subcommand by name; the daemon re-execs
//! itself for a profile apply through [`command`], so a tweak can neither share
//! state with the binder threads nor block them, exactly like the old
//! `systemv(...)` calls did.
//!
//! Each command is one module, named after the subcommand it answers to:
//!
//! | Module | Subcommand | Old binary |
//! |---|---|---|
//! | [`settings`] | `profilesettings` | `sys.azenith-profilesettings` |
//! | [`utility`] | `utilityconf` | `sys.azenith-utilityconf` |
//! | [`prefs`] | `preferencedtweaks` | `sys.azenith-preferencedtweaks` |
//! | [`preload`] | `preload` | `sys.azenith-preloadbin` |
//!
//! Cross-cutting helpers (log writer, properties, sysfs writes, forked shell)
//! live in [`crate::common`], not here.

pub(crate) mod prefs;
pub(crate) mod preload;
pub(crate) mod settings;
pub(crate) mod utility;

use std::fs;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::common::log::{comp, Lv, log_write};

pub(crate) const PROFILESETTINGS: &str = "profilesettings";
pub(crate) const UTILITYCONF: &str = "utilityconf";
pub(crate) const PREFERENCEDTWEAKS: &str = "preferencedtweaks";

/// Build a `Command` that re-runs this very executable as `<name> ...`.
///
/// `current_exe()` is `/proc/self/exe`, so the subcommand works no matter how
/// the daemon was started (module path, symlink, `--run` from service.sh) —
/// no `PATH` lookup and no hard-coded module directory. `argv[0]` is set to
/// `sys.azenith-service` rather than the real path so that a subcommand which
/// spawns another one (`initialize` -> `utilityconf` / `preferencedtweaks`)
/// still presents a parent [`verify_caller`] accepts.
pub(crate) fn command(name: &str) -> Command {
    let exe = std::env::current_exe().unwrap_or_else(|e| {
        log_write(Lv::Warn, comp::CMD, &format!("cannot resolve own executable ({e}); let PATH find the service"));
        std::path::PathBuf::from("sys.azenith-service")
    });

    let mut cmd = Command::new(exe);
    cmd.arg0("sys.azenith-service");
    cmd.arg(name);
    cmd
}

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

/// `true` when the parent process is the service — the guard the profile and
/// preference commands have always had. Both are only ever reached through
/// [`command`], so a human invoking them from a shell has a shell as the parent
/// and is rejected.
pub(crate) fn verify_caller() -> bool {
    let Some(cmdline) = parent_pid().and_then(process_cmdline) else { return false };
    // `azenith-service` covers both the installed name
    // (`/vendor/bin/hw/vendor.azenith-service`, what init runs) and the
    // `sys.azenith-service` argv[0] [`command`] gives a re-exec'd child; the
    // legacy `sys.azenith*` binary names are still accepted for older setups.
    cmdline.contains("azenith-service") || cmdline.contains("sys.azenith")
}
