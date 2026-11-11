// ---------------- profiler (profile switching, mirrors jni/src/profile/* in AZenith) ----------------
// Profiles write API/current_profile + API/gameinfo and then run the profile
// engine in `cmd::settings` directly. The mode decision is made from binder
// events: a gamelist app in the foreground (IProcessObserver) -> performance,
// battery saver -> eco, everything else -> balanced, with the 10s screen-off
// grace period of the C loop.
use std::collections::BTreeSet;
use std::ffi::c_char;
use std::fs;
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::common::log::{comp, debug, info};
use crate::daemon::state::{saver, screen, write_atomic};

// Digenerate dari Config.toml (lihat Cargo/build.rs dan crate::common::paths).
pub(crate) use crate::common::paths::{CURRENT_PROFILE as PROFILE_FILE, GAMEINFO as GAMEINFO_FILE};

// ProfileMode in AZenith.h
pub(crate) const MODE_PERF: u8 = 1;
pub(crate) const MODE_BALANCED: u8 = 2;
pub(crate) const MODE_ECO: u8 = 3;

const SCREEN_OFF_GRACE: Duration = Duration::from_secs(10);

static CUR: Mutex<u8> = Mutex::new(0); // 0 = nothing applied yet
static GRACE: Mutex<Option<Instant>> = Mutex::new(None); // screen-off grace period
static TX: Mutex<Option<mpsc::Sender<u8>>> = Mutex::new(None); // profile worker

/// Returns true when `persist.sys.azenithIntg.disabletweak` is "1".
/// When disabled, the profile mode is still decided and written to
/// `current_profile`, but no sysfs/governor/tweak writes happen — including
/// the `initialize` boot phase.
fn disable_tweak() -> bool {
    crate::common::prop::getprop("persist.sys.azenithIntg.disabletweak") == "1"
}

// Worker that applies profiles. A profile writes sysfs and forks helper
// commands, so running it inline would block a binder callback and hold the
// state lock for the whole run; queue it on a dedicated thread instead.
pub(crate) fn init() {
    info!(comp::PROFILER, "profiler initialized");
    let (tx, rx) = mpsc::channel::<u8>();
    *TX.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    thread::spawn(move || {
        let mut last = 0u8;
        while let Ok(mode) = rx.recv() {
            if mode == last { continue; }
            last = mode;
            run_profile(mode);
        }
    });
}

fn queue_profile(mode: u8) {
    let tx = TX.lock().unwrap_or_else(|e| e.into_inner()).as_ref().cloned();
    match tx {
        Some(t) => { let _ = t.send(mode); }
        None => run_profile(mode), // init() not called yet
    }
}

pub(crate) fn mode_name(m: u8) -> &'static str {
    match m {
        0 => "none",
        MODE_PERF => "performance",
        MODE_BALANCED => "balanced",
        MODE_ECO => "eco",
        _ => "unknown",
    }
}

// Name of the profile currently applied (for state.toml).
pub(crate) fn cur_name() -> &'static str {
    mode_name(*CUR.lock().unwrap_or_else(|e| e.into_inner()))
}

fn time_str() -> String {
    unsafe {
        let mut t: libc::timespec = std::mem::zeroed();
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut t);
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t.tv_sec, &mut tm);
        let mut buf = [0u8; 32];
        let n = libc::strftime(buf.as_mut_ptr() as *mut c_char, buf.len(), c"%H:%M:%S".as_ptr(), &tm);
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

fn uid_of(pid: i32) -> i32 {
    fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Uid:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok()))
        .unwrap_or(0)
}

// gameinfo: "<pkg> <pid> <uid>\nTime: HH:MM:SS\n" or "NULL 0 0\nTime: HH:MM:SS\n"
fn write_gameinfo(active: &Option<(String, BTreeSet<i32>)>) {
    let t = time_str();
    let body = match active {
        Some((pkg, pids)) => {
            let pid = pids.iter().next().copied().unwrap_or(0);
            format!("{pkg} {pid} {}\nTime: {t}\n", uid_of(pid))
        }
        None => format!("NULL 0 0\nTime: {t}\n"),
    };
    write_atomic(GAMEINFO_FILE, &body);
}

// The profiler functions live in `cmd::settings`; call them in this process so
// the daemon stays one binary and profile state (governor, debug flags) is
// shared instead of being lost between re-exec'd children.
fn run_profile(mode: u8) {
    if disable_tweak() {
        info!(comp::PROFILER, "disabletweak=1, skipping profile apply mode={}", mode_name(mode));
        return;
    }
    let t = Instant::now();
    crate::cmd::settings::apply_mode(mode);
    info!(comp::PROFILER, "profile apply finished mode={} in {:.0?}", mode_name(mode), t.elapsed());
}

// Boot phase: mode 0 — the governor/IO/Mali init, thermal, networking,
// kernel/VM tweaks, FSTrim and prefs that the one-shot shell script
// (AZenith_config) used to run. The daemon owns it now so a fork without that
// script still gets it.
//
// Called once from `main` *before the binder thread pool starts*, on purpose:
// after that a callback can reach `queue_profile`, which either hands the
// profile to the worker (racing this call) or runs it synchronously on the
// callback thread (because `TX` is not set yet) — either way a profile could be
// applied before `initialize` had a chance to seed the governor defaults.
pub(crate) fn run_initialize() {
    run_profile(0);
}

fn apply(mode: u8, active: &Option<(String, BTreeSet<i32>)>, why: &str) {
    let from = {
        let mut c = CUR.lock().unwrap_or_else(|e| e.into_inner());
        let f = *c;
        *c = mode;
        f
    };
    info!(comp::PROFILER, "profile: [{}] -> [{}] ({why})", mode_name(from), mode_name(mode));
    write_gameinfo(active);
    write_atomic(PROFILE_FILE, &format!("{mode}\n"));
    queue_profile(mode);
    broadcast_profile_change(mode, active);
}

fn broadcast_profile_change(mode: u8, active: &Option<(String, BTreeSet<i32>)>) {
    let mode_str = mode_name(mode);
    let pkg_arg = match active {
        Some((pkg, _)) if !pkg.is_empty() && pkg != "NULL" => format!(" --es pkg {pkg}"),
        _ => String::new(),
    };
    let cmd = format!(
        "am broadcast --user 0 -a com.rianixia.settings.SHOW_NOTIFICATION -n com.rianixia.settings.overlay/com.rianixia.settings.overlay.OverlayService --es category azenith --es profile {mode_str}{pkg_arg} &"
    );
    std::thread::spawn(move || {
        let _ = crate::common::shell::system(comp::PROFILER, &cmd);
    });
}

// Decide the profile from the current state. Called after every foreground/died/status change.
pub(crate) fn checkup(active: &Option<(String, BTreeSet<i32>)>, why: &str) {
    let cur = *CUR.lock().unwrap_or_else(|e| e.into_inner());
    let screen_off = screen() == "off";
    let saver_on = saver() == "on";

    let want = if active.is_some() && !screen_off {
        MODE_PERF
    } else if saver_on {
        MODE_ECO
    } else {
        MODE_BALANCED
    };

    // Screen off while a game is running: keep performance for 10s (grace period).
    let mut grace = GRACE.lock().unwrap_or_else(|e| e.into_inner());
    if cur == MODE_PERF && want != MODE_PERF && active.is_some() && screen_off {
        match *grace {
            None => {
                *grace = Some(Instant::now());
                info!(comp::PROFILER, "screen off grace period started (timeout=10s, keep performance)");
            }
            Some(t) => debug!(comp::PROFILER, "grace period active {:.0}s ({why})", t.elapsed().as_secs_f32()),
        }
        return;
    }
    *grace = None;

    if want == cur {
        debug!(comp::PROFILER, "profile unchanged [{}] ({why})", mode_name(want));
        return;
    }
    apply(want, active, why);
}

// Called from the status poll cycle: drops performance once the grace period expires.
pub(crate) fn tick() {
    let expired = {
        let g = GRACE.lock().unwrap_or_else(|e| e.into_inner());
        matches!(*g, Some(t) if t.elapsed() >= SCREEN_OFF_GRACE)
    };
    if !expired { return; }
    info!(comp::PROFILER, "grace period expired, dropping performance");
    *GRACE.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let mut g = crate::daemon::state::st();
    crate::daemon::state::apply(&mut *g, "grace period expired");
}
