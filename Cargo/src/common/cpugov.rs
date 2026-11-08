//! CPU governor the daemon saved before a profile overwrote it.
//!
//! Per-profile governor resolution works like this now:
//!
//! * `performance` / `eco` — if `custom_performance_cpu_gov` /
//!   `custom_powersave_cpu_gov` is set it wins. Otherwise the governor currently
//!   on cpu0 is saved (see [`save`]) and the mode's own governor (`performance` /
//!   `powersave`) is applied.
//! * `balanced` — `custom_default_cpu_gov` wins when set; otherwise the saved
//!   governor is restored. With nothing ever saved (first apply after boot) the
//!   kernel baseline is left untouched.
//!
//! The saved value lives in a file in `API_DIR` (see
//! [`crate::common::paths::SAVED_GOVERNOR`]). Profiles now run in-process, but
//! persisting the baseline means a daemon restart while `performance`/`eco` is
//! active can still restore it when the next `balanced` apply runs. The file is
//! tagged with the kernel boot ID: a value left over from a previous boot is
//! ignored, so the first `balanced` apply after a reboot still adopts whatever
//! governor the kernel came up with.

use std::fs;

use crate::common::log::{comp, warn};
use crate::common::paths::SAVED_GOVERNOR;

fn boot_id() -> String {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// The governor to go back to, or `""` when nothing has been saved for this boot.
pub(crate) fn saved() -> String {
    let Ok(raw) = fs::read_to_string(SAVED_GOVERNOR) else { return String::new() };
    let boot = boot_id();
    let mut lines = raw.lines();
    if boot.is_empty() || lines.next().map(str::trim) != Some(boot.as_str()) {
        return String::new();
    }
    lines.next().unwrap_or_default().trim().to_string()
}

/// Remember `gov` as the governor to restore, overwriting any previous value.
///
/// Called on every entry into `performance`/`eco`, so what is held is always the
/// governor that was in place *just before this mode switch*. Every mode change
/// therefore re-anchors `balanced` to the governor the kernel was on between two
/// modes — not to whatever the first switch after boot saw.
pub(crate) fn save(gov: &str) {
    let gov = gov.trim();
    if gov.is_empty() { return; }

    let boot = boot_id();
    if boot.is_empty() {
        warn!(comp::SETTINGS_CPU, "cannot read boot_id, skipping governor save");
        return;
    }

    // Write + rename so a concurrent read never sees a half-written file.
    // Profile applies are serialized by the daemon worker, but the extra step
    // also covers a service restart racing a profile switch.
    let tmp = format!("{SAVED_GOVERNOR}.{}.tmp", std::process::id());
    let body = format!("{boot}\n{gov}\n");
    match fs::write(&tmp, body).and_then(|_| fs::rename(&tmp, SAVED_GOVERNOR)) {
        Ok(()) => {}
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            warn!(comp::SETTINGS_CPU, "failed to save governor to {SAVED_GOVERNOR}: {e}");
        }
    }
}
