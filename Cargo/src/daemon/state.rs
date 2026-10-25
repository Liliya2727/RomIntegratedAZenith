// ---------------- profile state + state.toml (state store, like the config store in encore) ----------------
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use crate::common::log::{comp, debug, error, info};
use crate::daemon::profiler;
use crate::daemon::procs::{self, scan_apps};

// Digenerate dari Config.toml (lihat Cargo/build.rs dan crate::common::paths).
use crate::common::paths::STATE_FILE;

pub(crate) struct St {
    pub(crate) list: BTreeSet<String>,                  // packages in the gamelist
    pub(crate) active: Option<(String, BTreeSet<i32>)>, // (package, live pids) of the active app
    pub(crate) focused: Option<(String, i32, i32)>,     // (package, pid, uid) of foreground app
    pub(crate) procs: BTreeMap<i32, (String, i32)>,     // pid -> (package, uid) of all known app processes
    last: String,                                       // last active app recorded in state.toml
}

static ST: Mutex<St> = Mutex::new(St {
    list: BTreeSet::new(),
    active: None,
    focused: None,
    procs: BTreeMap::new(),
    last: String::new(),
});

pub(crate) fn st() -> MutexGuard<'static, St> { ST.lock().unwrap_or_else(|e| e.into_inner()) }

pub(crate) fn is_app(uid: i32) -> bool { (10000..20000).contains(&(uid % 100000)) }

pub(crate) fn pkg_of_pid(pid: i32) -> Option<String> {
    st().procs.get(&pid).map(|(p, _)| p.clone())
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

pub(crate) fn add_proc(pid: i32, pkg: &str, uid: i32) {
    let Some(pkg) = procs::base_pkg(pkg) else { return };
    let mut g = st();
    let s = &mut *g;
    s.procs.insert(pid, (pkg.clone(), uid));
    if let Some((active_pkg, pids)) = &mut s.active {
        if *active_pkg == pkg {
            pids.insert(pid);
        }
    }
}

pub(crate) fn remove_proc(pid: i32, why: &str) {
    let mut g = st();
    let s = &mut *g;
    s.procs.remove(&pid);
    if let Some((_, fpid, _)) = s.focused {
        if fpid == pid {
            s.focused = None;
        }
    }
    let gone = match &mut s.active {
        Some((_, pids)) => pids.remove(&pid) && pids.is_empty(),
        None => false,
    };
    if gone {
        if let Some((pkg, _)) = s.active.take() {
            info!(comp::STATE, "active app died: {pkg} (pid {pid})");
        }
        apply(s, why);
    }
}

pub(crate) fn set_focused(pkg: &str, pid: i32, uid: i32, why: &str) {
    let pkg = procs::base_pkg(pkg).unwrap_or_else(|| pkg.to_string());
    let mut g = st();
    let s = &mut *g;
    s.focused = Some((pkg.clone(), pid, uid));
    s.procs.insert(pid, (pkg.clone(), uid));

    if s.list.contains(&pkg) {
        let same = matches!(&s.active, Some((p, _)) if *p == pkg);
        if same {
            if let Some((_, pids)) = &mut s.active { pids.insert(pid); }
        } else {
            info!(comp::STATE, "gamelist app went to foreground: {pkg} (pid {pid})");
            s.active = Some((pkg, BTreeSet::from([pid])));
            apply(s, why);
        }
    } else if let Some((prev_pkg, _)) = s.active.take() {
        info!(comp::STATE, "leaving game {prev_pkg} -> non-game fg: {pkg} (pid {pid})");
        apply(s, why);
    }
}

pub(crate) fn seed() {
    let apps = scan_apps();
    let mut g = st();
    let s = &mut *g;
    for a in &apps {
        s.procs.insert(a.pid, (a.pkg.clone(), a.uid));
    }
    let mut running: Vec<&procs::AppProc> = apps.iter().collect();
    running.sort_by_key(|a| (std::cmp::Reverse(a.adj), a.pid));
    let top: Vec<&procs::AppProc> = running.iter().copied().filter(|a| a.adj == 0).collect();
    if top.len() == 1 {
        let a = top[0];
        info!(comp::STATE, "initial state: [{}] is in foreground (pid {}, oom_score_adj 0)", a.pkg, a.pid);
        s.focused = Some((a.pkg.clone(), a.pid, a.uid));
        if s.list.contains(&a.pkg) {
            s.active = Some((a.pkg.clone(), BTreeSet::from([a.pid])));
        }
    } else {
        debug!(comp::STATE, "initial state: no unambiguous foreground app ({} candidates)", top.len());
    }
    info!(comp::STATE, "process table seeded with {} app processes", apps.len());
    apply(s, "startup");
}

pub(crate) fn scan_foreground(list: &BTreeSet<String>) -> Option<(i32, String)> {
    let apps = scan_apps();
    apps.into_iter().find(|a| a.adj == 0 && list.contains(&a.pkg)).map(|a| (a.pid, a.pkg))
}

pub(crate) fn adopt(why: &str) {
    let mut g = st();
    let s = &mut *g;
    if s.active.is_none() {
        if let Some((ref pkg, pid, _)) = s.focused {
            if s.list.contains(pkg) {
                s.active = Some((pkg.clone(), BTreeSet::from([pid])));
            }
        }
    }
    apply(s, why);
}

pub(crate) fn watch_processes() {
    thread::spawn(|| loop {
        thread::sleep(Duration::from_millis(1500));
        let apps = scan_apps();
        let top = apps.iter().find(|a| a.adj == 0);
        if let Some(top) = top {
            let mut g = st();
            let s = &mut *g;
            let current_fg = s.focused.as_ref().map(|(p, _, _)| p.clone());
            if current_fg.as_deref() != Some(&top.pkg) {
                drop(g);
                set_focused(&top.pkg, top.pid, top.uid, "process poller");
                continue;
            }
        }
        // If an active game is set, verify its processes are still foreground
        let mut g = st();
        let s = &mut *g;
        if let Some((ref game_pkg, ref pids)) = s.active {
            let still_fg = apps.iter().any(|a| a.pkg == *game_pkg && a.adj <= 200 && pids.contains(&a.pid));
            if !still_fg && !apps.iter().any(|a| a.pkg == *game_pkg && pids.contains(&a.pid)) {
                let pkg = game_pkg.clone();
                s.active = None;
                info!(comp::STATE, "active game {pkg} is no longer running in foreground");
                apply(s, "process poller: game gone");
            }
        }
    });
}
