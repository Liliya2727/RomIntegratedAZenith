// ---------------- /proc scan: app process table + initial foreground guess ----------------
// Adapted from sysdaemon in azenith (procs.rs).
use std::fs;
use std::os::unix::fs::MetadataExt;

use crate::daemon::state::is_app;

#[derive(Clone, Debug)]
pub(crate) struct AppProc {
    pub(crate) pid: i32,
    pub(crate) pkg: String,
    pub(crate) uid: i32,
    /// oom_score_adj: 0 means "top app" (the app the user is looking at).
    pub(crate) adj: i32,
}

/// Base package of a process name: "com.foo.bar:remote" -> "com.foo.bar".
/// Shells, native binaries and zygote helpers are rejected.
pub(crate) fn base_pkg(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') { return None; }
    let base = name.split(':').next().unwrap_or(name);
    if !base.contains('.') || base.starts_with('.') || base.ends_with("_zygote") { return None; }
    Some(base.to_string())
}

/// Package of a running process, read from /proc/<pid>/cmdline.
pub(crate) fn pkg_of(pid: i32) -> Option<String> {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let name = String::from_utf8_lossy(raw.split(|&b| b == 0).next()?).into_owned();
    base_pkg(&name)
}

pub(crate) fn adj_of(pid: i32) -> i32 {
    fs::read_to_string(format!("/proc/{pid}/oom_score_adj"))
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(999)
}

/// Every running process that belongs to a user app.
pub(crate) fn scan_apps() -> Vec<AppProc> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir("/proc") else { return out; };
    for e in rd.flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() else { continue };
        let Ok(md) = e.metadata() else { continue };
        let uid = md.uid() as i32;
        if !is_app(uid) { continue; }
        let Some(pkg) = pkg_of(pid) else { continue };
        out.push(AppProc { pid, pkg, uid, adj: adj_of(pid) });
    }
    out
}
