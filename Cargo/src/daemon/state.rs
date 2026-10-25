// ---------------- profile state + state.toml (state store, like the config store in encore) ----------------
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::common::log::{comp, debug, error, info};
use crate::daemon::profiler;

// Digenerate dari Config.toml (lihat Cargo/build.rs dan crate::common::paths).
use crate::common::paths::STATE_FILE;

pub(crate) struct St {
    pub(crate) list: BTreeSet<String>,                    // packages in the gamelist
    pub(crate) active: Option<(String, BTreeSet<i32>)>,   // (package, live pids) of the active app
    last: String,                                         // last active app recorded in state.toml
}
static ST: Mutex<St> = Mutex::new(St { list: BTreeSet::new(), active: None, last: String::new() });
pub(crate) fn st() -> MutexGuard<'static, St> { ST.lock().unwrap_or_else(|e| e.into_inner()) }

pub(crate) fn is_app(uid: i32) -> bool { (10000..20000).contains(&(uid % 100000)) }

// Only used in scan_foreground (start / gamelist change), not on the hot path.
fn pkg_of_pid(pid: i32) -> Option<String> {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let name = String::from_utf8_lossy(raw.split(|&b| b == 0).next()?).into_owned();
    (!name.is_empty() && !name.contains(':') && name.contains('.')).then_some(name)
}

pub(crate) fn write_atomic(path: &str, data: &str) -> bool {
    // Unique tmp name per call — with a fixed name, two threads writing at the same
    // time could eat each other's tmp file during rename (ENOENT error).
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let tmp = format!("{path}.{}.tmp", SEQ.fetch_add(1, Ordering::Relaxed));
    match fs::write(&tmp, data).and_then(|_| fs::rename(&tmp, path)) {
        Ok(()) => true,
        Err(e) => { error!(comp::STATE, "failed to write {path}: {e}"); false }
    }
}

// ---------------- state.toml: profile + system status in a single file ----------------
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SysStat {
    pub(crate) screen: &'static str,  // on | off | unknown
    pub(crate) saver: &'static str,   // on | off | unknown
    pub(crate) zen: &'static str,     // off | priority | none | alarms | unknown
    pub(crate) charge: &'static str,  // charging | discharging | not_charging | full | unknown
    pub(crate) level: i32,            // 0..=100, -1 = unknown
}
const UNKNOWN_STAT: SysStat = SysStat { screen: "unknown", saver: "unknown", zen: "unknown", charge: "unknown", level: -1 };
static SYS: Mutex<SysStat> = Mutex::new(UNKNOWN_STAT);
static LAST_STATE: Mutex<String> = Mutex::new(String::new());

pub(crate) fn screen() -> &'static str { SYS.lock().unwrap_or_else(|e| e.into_inner()).screen }
pub(crate) fn saver() -> &'static str { SYS.lock().unwrap_or_else(|e| e.into_inner()).saver }

// Write only when the contents change.
fn write_state(pkg: &str, why: &str) {
    let s = SYS.lock().unwrap_or_else(|e| e.into_inner());
    let body = format!(
        "# state (auto-generated, do not edit manually)\n\
         profile = \"{}\"\n\
         active_app = \"{pkg}\"\n\
         screen = \"{screen}\"\n\
         battery_saver = \"{saver}\"\n\
         zen_mode = \"{zen}\"\n\
         battery_level = {level}\n\
         charging = \"{charge}\"\n",
        profiler::cur_name(),
        screen = s.screen, saver = s.saver, zen = s.zen, level = s.level, charge = s.charge,
    );
    drop(s);
    let mut last = LAST_STATE.lock().unwrap_or_else(|e| e.into_inner());
    if *last == body { debug!(comp::STATE, "state.toml unchanged, skipping write [{why}]"); return; }
    if write_atomic(STATE_FILE, &body) {
        *last = body;
        info!(comp::STATE, "state.toml updated ({why})");
    }
}

fn active_pkg(a: &Option<(String, BTreeSet<i32>)>) -> String {
    a.as_ref().map(|(p, _)| p.clone()).unwrap_or_else(|| "NULL".to_string())
}

// Update one status field; if it changed, rewrite state.toml and re-check the profile.
pub(crate) fn sys_update(why: &str, f: impl FnOnce(&mut SysStat)) {
    let changed = {
        let mut g = SYS.lock().unwrap_or_else(|e| e.into_inner());
        let before = *g;
        f(&mut *g);
        before != *g
    };
    if !changed { return; }
    let active = st().active.clone();
    let pkg = active_pkg(&active);
    // screen / battery saver can change the profile (eco, grace period)
    profiler::checkup(&active, why);
    write_state(&pkg, why);
}

// Update the active app, then let the profiler decide the profile (1/2/3).
pub(crate) fn apply(s: &mut St, why: &str) {
    let pkg = active_pkg(&s.active);
    if pkg != s.last {
        let from = if s.last.is_empty() { "(none)" } else { s.last.as_str() };
        info!(comp::STATE, "active app: [{from}] -> [{pkg}] ({why})");
        s.last = pkg.clone();
    } else {
        debug!(comp::STATE, "active app unchanged ({pkg}) [{why}]");
    }
    profiler::checkup(&s.active, why);
    write_state(&pkg, why);
}

pub(crate) fn sync_state(why: &str) {
    let active = st().active.clone();
    let pkg = active_pkg(&active);
    write_state(&pkg, why);
}

// App from the gamelist currently on screen (oom_score_adj 0). Only called on start / gamelist change.
//
// Note that this is a third answer to "is this package a game?", next to the
// first-boot scan in `daemon::apps` (which skips `zx.azenith` by name) and the
// `gamelist.txt` the user edits: whatever ends up in the list, file ownership is
// the only one the daemon acts on.
pub(crate) fn scan_foreground(list: &BTreeSet<String>) -> Option<(i32, String)> {
    let rd = fs::read_dir("/proc").ok()?;
    for e in rd.flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() else { continue };
        let Ok(md) = e.metadata() else { continue };
        if !is_app(md.uid() as i32) { continue; }
        let Some(pkg) = pkg_of_pid(pid) else { continue };
        if !list.contains(&pkg) { continue; }
        let top = fs::read_to_string(format!("/proc/{pid}/oom_score_adj"))
            .map(|v| v.trim() == "0").unwrap_or(false);
        if top { return Some((pid, pkg)); }
    }
    None
}

pub(crate) fn adopt(why: &str) {
    let mut g = st();
    let s = &mut *g;
    if s.active.is_none() {
        s.active = scan_foreground(&s.list).map(|(pid, pkg)| (pkg, BTreeSet::from([pid])));
        match &s.active {
            Some((pkg, pids)) => info!(comp::STATE, "initial scan: {pkg} (pid {pids:?}) is in the foreground"),
            None => debug!(comp::STATE, "initial scan: no gamelist app in the foreground"),
        }
    }
    apply(s, why);
}
