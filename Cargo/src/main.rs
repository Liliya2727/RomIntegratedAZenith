// libBinding — AZenith status & profile daemon, pure binder (no shell fork).
//
//   daemon/   the binder engine (ndk, codes, observer, applist, status, state, profiler)
//   cmd/      the companion commands merged into this binary (profilesettings, ...)
//   common/   log writer, property/fs/shell helpers shared by both
mod cmd;
mod common;
mod daemon;

use std::fs;
use std::ptr::null_mut;

use crate::cmd::preload::{self, PRELOAD};
use crate::common::log::{comp, debug, error, info, level_name, log_init, warn};
use crate::common::paths::{API_DIR, BASE_DIR, DAEMON_LOCK, GAMELIST_DIR};
use crate::daemon::applist::{reload_list, watch_list};
use crate::daemon::codes::{codes, load_codes, set_codes};
use crate::daemon::ndk::{call, class, n, service};
use crate::daemon::observer::{on_am_died, on_observer, set_pm};
use crate::daemon::state::adopt;
use crate::daemon::status::init_status;

/// Subcommand names accepted as the first word (`sys.azenith-service preload ...`),
/// plus the old binary names kept working for muscle memory:
/// `sys.azenith-profilesettings` -> `sys.azenith-service profilesettings`.
fn subcommand(name: &str) -> Option<&'static str> {
    Some(match name {
        "profilesettings" | "settings" | "profile" | "sys.azenith-profilesettings" => cmd::PROFILESETTINGS,
        "utilityconf" | "utility" | "sys.azenith-utilityconf" => cmd::UTILITYCONF,
        "preferencedtweaks" | "prefs" | "preferenced" | "sys.azenith-preferencedtweaks" => cmd::PREFERENCEDTWEAKS,
        "preload" | "preloadbin" | "sys.azenith-preloadbin" => PRELOAD,
        _ => return None,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Pick up `LIBBINDING_LOG` / `persist.sys.azenithIntg.debugmode` before anything can
    // log: the companion commands run before the daemon init below, and without
    // this their DEBUG lines were dropped by the default Info level.
    log_init();

    // Companion commands first: they run as the same binary but in their own
    // process, so nothing below (root check, instance lock, binder) applies.
    if let Some(name) = args.first().and_then(|a| subcommand(a)) {
        let rest = &args[1..];
        match name {
            cmd::PROFILESETTINGS => cmd::settings::run(rest),
            cmd::UTILITYCONF => cmd::utility::run(rest),
            cmd::PREFERENCEDTWEAKS => cmd::prefs::run(rest),
            PRELOAD => preload::run(rest),
            _ => unreachable!(),
        }
        return;
    }

    if unsafe { libc::getuid() } != 0 { eprintln!("run as root"); std::process::exit(1); }
    let _ = fs::create_dir_all(BASE_DIR);
    let _ = fs::create_dir_all(API_DIR);
    let _ = fs::create_dir_all(GAMELIST_DIR);

    // single instance: two daemons would fight over the profile
    use fs2::FileExt;
    let lock_path = DAEMON_LOCK.to_string();
    let lock = fs::OpenOptions::new().create(true).write(true).truncate(false).open(&lock_path);
    match lock {
        Ok(f) => {
            if f.try_lock_exclusive().is_err() {
                eprintln!("another instance is already running");
                std::process::exit(1);
            }
            std::mem::forget(f); // keep the lock for the whole process lifetime
        }
        Err(e) => warn!(comp::DAEMON, "cannot open {lock_path}: {e} (running without instance lock)"),
    }
    info!(comp::DAEMON, "daemon starting (pid {}, log level {})", std::process::id(), level_name());

    let Some(c) = load_codes() else {
        error!(comp::DAEMON, "failed to resolve transaction codes (check binder_resolver.apk)");
        std::process::exit(1);
    };
    set_codes(c);

    reload_list(); // initial load
    watch_list();  // auto reload when the file changes

    let n = n();
    let am = service(c"activity", c"android.app.IActivityManager");
    if am.is_null() { std::process::exit(1); }

    let pm = service(c"package", c"android.content.pm.IPackageManager");
    if pm.is_null() { std::process::exit(1); }
    set_pm(pm);

    unsafe {
        let rec = (n.AIBinder_DeathRecipient_new)(on_am_died);
        let r = (n.AIBinder_linkToDeath)(am, rec, null_mut());
        if r == 0 { debug!(comp::DAEMON, "linkToDeath registered on ActivityManager"); }
        else { warn!(comp::DAEMON, "linkToDeath failed, status={r}"); }
    }

    let obs = unsafe { (n.AIBinder_new)(class(c"android.app.IProcessObserver", on_observer), null_mut()) };
    if obs.is_null() { error!(comp::DAEMON, "failed to create observer binder"); std::process::exit(1); }

    let Some(reply) = call(am, codes().reg, |i| unsafe {
        (n.AParcel_writeStrongBinder)(i, obs);
    }) else {
        error!(comp::DAEMON, "registerProcessObserver denied/failed (wrong code for this ROM?)");
        std::process::exit(1);
    };
    unsafe { (n.AParcel_delete)(reply); }
    info!(comp::DAEMON, "IProcessObserver registered with ActivityManager");

    // Boot phase (`profilesettings 0`) first: it seeds the default governor/IO/Mali
    // properties that every profile below resolves its fallback chain from, so a
    // profile applied before it would read empty props and fall back to hardcoded
    // defaults. Before `startThreadPool` because no inbound binder transaction is
    // dispatched until then — see daemon::profiler::run_initialize. It also fills
    // `gamelist.txt` on the first boot.
    daemon::profiler::run_initialize();

    unsafe { (n.ABinderProcess_startThreadPool)(); }
    daemon::profiler::init(); // worker that applies profiles via the profilesettings subcommand
    init_status(); // register binder callbacks (screen/saver/zen) + read initial status
    adopt("start"); // write initial state (profile 1/2/3, or 2/NULL if no game is on screen)
    info!(comp::DAEMON, "ready, waiting for callbacks from AMS");
    unsafe { (n.ABinderProcess_joinThreadPool)(); }
}
