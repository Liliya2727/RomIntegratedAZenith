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
    pub(crate) active: Option<(String, BTreeSet<i32>)>, // selected game, retained until its last process dies
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

/// Return every process currently known for a package. Track all of them so
/// the game remains selected until the package's final process dies.
fn package_pids(s: &St, pkg: &str) -> BTreeSet<i32> {
    s.procs.iter()
        .filter_map(|(pid, (process_pkg, _))| (process_pkg == pkg).then_some(*pid))
        .collect()
}

pub(crate) fn remove_proc(pid: i32, why: &str) {
    let mut g = st();
    let s = &mut *g;
    let removed_pkg = s.procs.remove(&pid).map(|(pkg, _)| pkg);
    if let Some((_, fpid, _)) = s.focused {
        if fpid == pid {
            s.focused = None;
        }
    }
    let gone = if let Some((pkg, pids)) = &mut s.active
        && removed_pkg.as_deref() == Some(pkg.as_str())
    {
        pids.remove(&pid);
        !s.procs.values().any(|(process_pkg, _)| process_pkg == pkg)
    } else {
        false
    };
    if gone {
        if let Some((pkg, _)) = s.active.take() {
            info!(comp::STATE, "active game destroyed: {pkg} (last process died, pid {pid})");
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
            let pids = package_pids(s, &pkg);
            if let Some((_, active_pids)) = &mut s.active { active_pids.extend(pids); }
        } else {
            info!(comp::STATE, "game selected from foreground: {pkg} (pid {pid}); retaining until its last process dies");
            let pids = package_pids(s, &pkg);
            s.active = Some((pkg, pids));
            apply(s, why);
        }
    } else if let Some((active_pkg, _)) = &s.active {
        // Foreground is a detection signal, not the lifetime of a selected
        // game. Keep performance and the tracked package while it is
        // backgrounded; onProcessDied clears it after its last process exits.
        debug!(comp::STATE, "non-game foreground [{pkg}] while game [{active_pkg}] remains pinned until process death");
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
            let pids = package_pids(s, &a.pkg);
            s.active = Some((a.pkg.clone(), pids));
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
                let mut pids = package_pids(s, pkg);
                pids.insert(pid);
                s.active = Some((pkg.clone(), pids));
            }
        }
    }
    apply(s, why);
}

pub(crate) fn watch_processes() {
    thread::spawn(|| loop {
        thread::sleep(Duration::from_millis(1500));
        let apps = scan_apps();

        let mut g = st();
        let s = &mut *g;

        // AMS is the authority for the foreground app. oom_score_adj is only a
        // hint here: on some ROMs a background app can sit at 0 while a game is
        // at 100/200, so never let this poller replace a live AMS focus.
        let focused_alive = s.focused.as_ref()
            .map(|(_, pid, _)| apps.iter().any(|a| a.pid == *pid))
            .unwrap_or(false);
        if !focused_alive {
            s.focused = None;
        }

        // Recover only when AMS has not told us anything yet (startup or the
        // focused process died) and exactly one app claims the top slot. With
        // several adj==0 processes the result is ambiguous; leave it to AMS.
        if s.focused.is_none() && s.active.is_none() {
            let mut top = apps.iter().filter(|a| a.adj == 0);
            if let (Some(a), None) = (top.next(), top.next()) {
                let (pkg, pid, uid) = (a.pkg.clone(), a.pid, a.uid);
                drop(g);
                set_focused(&pkg, pid, uid, "process poller: recovery");
                continue;
            }
        }

        // Do not clear the selected game based on a /proc snapshot. AMS's
        // onProcessDied callback owns that lifecycle transition so a background
        // game keeps its profile until its last process has exited.
    });
}
