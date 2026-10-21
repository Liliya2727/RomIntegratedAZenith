//! Sysfs/procfs node writes for the `settings` command.
//!
//! Thin names for [`crate::common::fs`], kept because the profiles read better
//! with `write_lock` / `write_unlock` spelled out (does the node go back to
//! `0444` after the write?).

use crate::common::fs;
use crate::common::log::comp;

/// `chmod` under the name the profiles have always used; the implementation is
/// [`crate::common::fs::chmod`].
pub(crate) use crate::common::fs::chmod as common_chmod;

/// `chmod 644`, write, then keep the node unlocked.
pub(crate) fn write_unlock(value: &str, path_str: &str) {
    write(value, path_str, false);
}

/// `chmod 644`, write, then restore the node to `0444`.
pub(crate) fn write_lock(value: &str, path_str: &str) {
    write(value, path_str, true);
}

fn write(value: &str, path_str: &str, lock: bool) {
    fs::write_val(value, path_str, &node_name(path_str), lock, comp::SETTINGS_SYSFS);
}

/// `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` -> `cpu0/cpufreq/scaling_governor`,
/// so a log line names the node without repeating the whole sysfs prefix.
fn node_name(path_str: &str) -> String {
    let path = std::path::Path::new(path_str);
    let parent = path.parent().and_then(|p| p.file_name()).unwrap_or_default().to_string_lossy();
    let file = path.file_name().unwrap_or_default().to_string_lossy();
    if parent.is_empty() { file.into_owned() } else { format!("{}/{}", parent, file) }
}
