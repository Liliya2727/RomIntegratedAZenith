//! Filesystem helpers shared by the daemon and the companion commands.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::common::log::{comp, Lv, log_write};

/// `chmod` that warns instead of failing: a sysfs node the kernel refuses to
/// unlock must not abort the tweak that wanted to write it.
///
/// Two quieting rules, because the kernel nodes this runs against do not behave
/// like disk files:
///
/// * a chmod that would not change anything is skipped, so no syscall and no
///   line for a `/proc/sys/vm/*` node that already sits at its stock `0644`;
/// * `/proc/sys` takes its mode from the sysctl table and answers `chmod(2)`
///   with `EPERM` even as root while the node stays writable, so that refusal
///   is only a DEBUG line — the write in [`write_val`] reports the real outcome.
pub(crate) fn chmod(path: &str, mode: u32) {
    match fs::metadata(path) {
        Ok(metadata) => {
            let mut perms = metadata.permissions();
            if perms.mode() & 0o7777 == mode {
                return;
            }
            perms.set_mode(mode);
            if let Err(e) = fs::set_permissions(path, perms) {
                let refused = matches!(
                    e.raw_os_error(),
                    Some(libc::EPERM) | Some(libc::EACCES) | Some(libc::EROFS)
                );
                let lv = if refused { Lv::Debug } else { Lv::Error };
                log_write(lv, comp::COMMON_FS, &format!("set_permissions failed path={} err={}", path, e));
            }
        }
        Err(e) => log_write(Lv::Warn, comp::COMMON_FS, &format!("chmod metadata failed path={} err={}", path, e)),
    }
}

/// `chmod` over every path a glob matches (silently, like the tweaks expect).
pub(crate) fn chmod_glob(pattern: &str, mode: u32) {
    let Ok(paths) = glob::glob(pattern) else { return };
    for path in paths.flatten() {
        if let Some(p_str) = path.to_str() {
            chmod(p_str, mode);
        }
    }
}

/// Strip the well-known kernel roots so a log line names the node without
/// repeating the whole sysfs/procfs prefix:
///
/// ```text
/// /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor -> cpu0/cpufreq/scaling_governor
/// /sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq -> cpu0/cpufreq/scaling_max_freq
/// /proc/sys/vm/vfs_cache_pressure                     -> vm/vfs_cache_pressure
/// ```
///
/// The same convention the C tweaks used (`path=/cpu0/cpufreq/...`), so the
/// lines stay comparable with the old logs.
pub(crate) fn node_name(path_str: &str) -> String {
    for root in ["/sys/devices/system/", "/sys/devices/", "/sys/class/", "/sys/module/", "/sys/", "/proc/sys/", "/proc/"] {
        if let Some(rest) = path_str.strip_prefix(root) {
            return rest.to_string();
        }
    }
    path_str.trim_start_matches('/').to_string()
}

/// Write `value` to a sysfs/procfs path.
///
/// `node` is the path as it should appear in the log (`path=/cpu/0/cpufreq`, no
/// leading slash on the parent); `lock` restores `0444` after the write so the
/// kernel node goes back to read-only. Returns whether the write landed.
///
/// `component` is the caller's tag from [`crate::common::log::comp`], so the
/// line says which subsystem (`cmd.settings.sysfs`, `cmd.prefs.write`, ...)
/// touched the node.
pub(crate) fn write_val(value: &str, path_str: &str, node: &str, lock: bool, component: &str) -> bool {
    let path = Path::new(path_str);

    if !path.exists() {
        log_write(Lv::Debug, component, &format!("skipped path=/{} reason=not_found", node));
        return false;
    }

    chmod(path_str, 0o644);
    match fs::write(path, value) {
        Ok(()) => {
            if lock {
                chmod(path_str, 0o444);
            }
            log_write(Lv::Debug, component, &format!("set path=/{} value={}", node, value));
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            log_write(Lv::Warn, component, &format!("set failed path=/{} reason=permission_denied", node));
            false
        }
        Err(e) => {
            log_write(Lv::Error, component, &format!("set failed path=/{} err={}", node, e));
            false
        }
    }
}
