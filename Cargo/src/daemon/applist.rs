// ---------------- gamelist.txt (GameRegistry) + inotify (InotifyWatcher) ----------------
use std::collections::BTreeSet;
use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::thread;

use crate::common::log::{comp, debug, error, info, warn};
use crate::daemon::state::{adopt, st};

// Digenerate dari Config.toml (lihat Cargo/build.rs dan crate::common::paths).
pub(crate) use crate::common::paths::{GAMELIST_DIR as LIST_DIR, GAMELIST_FILE as LIST_FILE};
const LIST_NAME: &str = "gamelist.txt";

/// Marker in `GAMELIST_DIR` meaning "this file already holds what the device
/// has installed": scanning is a boot-once job, so the daemon does not repeat a
/// multi-second `cmd package` on every start.
const LOADED_FLAG: &str = "gamelistloaded";

// One package name per line. Blank lines, '#' comments and anything after whitespace
// are ignored, so "com.foo # shooter" and "com.foo" both give "com.foo".
fn parse_list(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .map(|l| l.split_whitespace().next().unwrap_or(""))
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

pub(crate) fn reload_list() {
    let text = match fs::read_to_string(LIST_FILE) {
        Ok(t) => t,
        Err(e) => { warn!(comp::APPLIST, "gamelist unreadable ({LIST_FILE}): {e}"); return; }
    };
    let new = parse_list(&text);
    let total = new.len();
    let (added, removed, dropped) = {
        let mut g = st();
        let s = &mut *g;
        let added: Vec<String> = new.difference(&s.list).cloned().collect();
        let removed: Vec<String> = s.list.difference(&new).cloned().collect();
        s.list = new;
        // active app was removed from the gamelist -> fallback
        let mut dropped = None;
        if matches!(&s.active, Some((p, _)) if !s.list.contains(p)) {
            dropped = s.active.take().map(|(p, _)| p);
        }
        (added, removed, dropped)
    };
    info!(comp::APPLIST, "gamelist loaded: {total} packages (+{} -{})", added.len(), removed.len());
    if !added.is_empty() { debug!(comp::APPLIST, "gamelist added: {}", added.join(", ")); }
    if !removed.is_empty() { debug!(comp::APPLIST, "gamelist removed: {}", removed.join(", ")); }
    if let Some(p) = dropped { info!(comp::APPLIST, "active app {p} removed from gamelist, falling back"); }
    debug!(comp::APPLIST, "full gamelist: {}", st().list.iter().cloned().collect::<Vec<_>>().join(", "));
    adopt("gamelist changed");
}

/// Overwrite `gamelist.txt` with `packages`, one per line. Only called when the
/// caller is sure the old contents should go away (first boot / manual rerun).
fn write_list(packages: &BTreeSet<String>) -> bool {
    let mut body = String::new();
    for pkg in packages {
        body.push_str(pkg);
        body.push('\n');
    }
    // Non-atomic on purpose: `write_atomic` renames into place, and the inotify
    // watch only reports IN_CLOSE_WRITE | IN_MOVED_TO — meaning a temp file that
    // would otherwise look like a second gamelist. The worker that calls this has
    // no other writer, so a plain write is enough.
    match fs::write(LIST_FILE, body) {
        Ok(()) => true,
        Err(e) => { error!(comp::APPLIST, "failed to write {LIST_FILE}: {e}"); false }
    }
}

/// Fill `gamelist.txt` from the package manager on first boot only.
///
/// The flag file is what makes this "first boot": the scan asks the package
/// manager for every installed package (2–10s on a full device), so it must not
/// run on every daemon start — and `initialize` runs on every start. No flag
/// means this device has not been scanned yet, so the list is rebuilt from
/// scratch; the flag is only written once the scan succeeded, so a failed
/// attempt is retried on the next boot.
///
/// A rerun without the daemon (re-exec `sys.azenith-service` the way init does)
/// works too: there is no lock file, the command works standalone, and the
/// daemon picks up the rewritten gamelist through its inotify watch.
pub(crate) fn init_gamelist() {
    if Path::new(&loaded_flag()).exists() {
        debug!(comp::APPLIST, "gamelist already loaded ({}), skipping scan", loaded_flag());
        return;
    }
    info!(comp::APPLIST, "first boot: building gamelist in {LIST_DIR}");

    let packages = match crate::daemon::apps::installed_packages() {
        Some(p) => p,
        None => { warn!(comp::APPLIST, "installed-package scan unavailable, will retry on the next boot"); return; }
    };
    info!(comp::APPLIST, "installed-package scan found {} packages", packages.len());
    if !write_list(&packages) {
        return;
    }
    let _ = fs::write(loaded_flag(), format!("{}\n", packages.len()));

    reload_list(); // reads back what was written and adopts whatever is on screen
    info!(comp::APPLIST, "gamelist ready: {} packages in {LIST_DIR}/{LIST_NAME}", packages.len());
}

fn loaded_flag() -> String {
    format!("{LIST_DIR}/{LOADED_FLAG}")
}

// inotify on the directory (editors often replace files via rename). The thread sleeps in read().
pub(crate) fn watch_list() {
    thread::spawn(|| unsafe {
        let fd = libc::inotify_init1(libc::IN_CLOEXEC);
        if fd < 0 { error!(comp::APPLIST, "inotify_init1 failed"); return; }
        let dir = std::ffi::CString::new(LIST_DIR).unwrap();
        let wd = libc::inotify_add_watch(fd, dir.as_ptr(), libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO);
        if wd < 0 { error!(comp::APPLIST, "inotify_add_watch failed: {LIST_DIR}"); return; }
        info!(comp::APPLIST, "watching for changes to {LIST_DIR}/{LIST_NAME}");

        let mut buf = [0u8; 4096];
        loop {
            let r = libc::read(fd, buf.as_mut_ptr() as *mut c_void, buf.len());
            if r < 0 && *libc::__errno() == libc::EINTR { continue; }
            if r <= 0 { error!(comp::APPLIST, "inotify read stopped (r={r})"); return; }
            let total = r as usize;
            let (mut off, mut hit) = (0usize, false);
            while off + 16 <= total {
                let len = u32::from_ne_bytes(buf[off + 12..off + 16].try_into().unwrap()) as usize;
                if off + 16 + len > total { break; }
                let name = buf[off + 16..off + 16 + len].split(|&b| b == 0).next().unwrap_or(&[]);
                if name == LIST_NAME.as_bytes() { hit = true; }
                off += 16 + len;
            }
            if hit {
                debug!(comp::APPLIST, "inotify: {LIST_NAME} changed");
                reload_list();
            }
        }
    });
}
