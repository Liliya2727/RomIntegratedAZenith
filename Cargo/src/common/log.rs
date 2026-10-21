// ---------------- logging ----------------
//
// One logcat writer for the whole service binary. The daemon, the binder
// callbacks and every companion command (`sys.azenith-service
// profilesettings ...`, `... utilityconf ...`, `... preferencedtweaks ...`,
// `... preload ...`) all emit through this module — nothing is written to
// disk anymore, so read the output with:
//
//   adb logcat -s AZenith
//
// `component` is a dotted path naming the owner of the message. The first
// segment is the part of the module that emitted it (`daemon`, `cmd`,
// `common`), the rest narrows it to a subsystem:
//
//   [daemon.profiler]   [daemon.status]   [cmd.settings.cpu]
//   [cmd.utility.shell] [cmd.prefs.walt]  [cmd.preload]
//
// Use the constants in [`comp`] rather than a literal, so a message can never
// drift into an ad-hoc tag.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::common::android_ffi;

/// logcat tag for every line this binary prints.
const LOGCAT_TAG: &str = "AZenith";

/// Property that turns DEBUG detail on (`true`). The `LIBBINDING_LOG`
/// environment variable overrides it for a single invocation.
const DEBUGMODE_PROP: &str = "persist.azenith.debugmode";

#[derive(Clone, Copy)]
#[repr(u8)]
pub(crate) enum Lv { Debug = 0, Info = 1, Warn = 2, Error = 3 }

static LEVEL: AtomicU8 = AtomicU8::new(Lv::Info as u8);

pub(crate) fn level_name() -> &'static str {
    ["debug", "info", "warn", "error"][LEVEL.load(Ordering::Relaxed) as usize]
}

/// Component tags, grouped by the module that owns them.
pub(crate) mod comp {
    // daemon: the binder engine itself.
    pub(crate) const DAEMON: &str = "daemon";
    pub(crate) const NDK: &str = "daemon.ndk";
    pub(crate) const CODES: &str = "daemon.codes";
    pub(crate) const STATUS: &str = "daemon.status";
    pub(crate) const OBSERVER: &str = "daemon.observer";
    pub(crate) const APPLIST: &str = "daemon.applist";
    pub(crate) const STATE: &str = "daemon.state";
    pub(crate) const PROFILER: &str = "daemon.profiler";

    // cmd.<command>.<subsystem>: the companion commands.
    pub(crate) const CMD: &str = "cmd";
    pub(crate) const PRELOAD: &str = "cmd.preload";
    pub(crate) const SETTINGS_CPU: &str = "cmd.settings.cpu";
    pub(crate) const SETTINGS_RENDERER: &str = "cmd.settings.renderer";
    pub(crate) const SETTINGS_SYSFS: &str = "cmd.settings.sysfs";
    pub(crate) const SETTINGS_PROP: &str = "cmd.settings.prop";
    pub(crate) const SETTINGS_APPS: &str = "cmd.settings.apps";
    pub(crate) const SETTINGS_PROFILE: &str = "cmd.settings.profile";
    pub(crate) const UTILITY_CPU: &str = "cmd.utility.cpu";
    pub(crate) const UTILITY_THERMAL: &str = "cmd.utility.thermal";
    pub(crate) const UTILITY_FSTRIM: &str = "cmd.utility.fstrim";
    pub(crate) const UTILITY_DND: &str = "cmd.utility.dnd";
    pub(crate) const UTILITY_REFRESH: &str = "cmd.utility.refresh";
    pub(crate) const UTILITY_RENDERER: &str = "cmd.utility.renderer";
    pub(crate) const UTILITY_SHELL: &str = "cmd.utility.shell";
    pub(crate) const PREFS_JIT: &str = "cmd.prefs.jit";
    pub(crate) const PREFS_SCHED: &str = "cmd.prefs.sched";
    pub(crate) const PREFS_WALT: &str = "cmd.prefs.walt";
    pub(crate) const PREFS_FPSGO: &str = "cmd.prefs.fpsgo";
    pub(crate) const PREFS_GPU: &str = "cmd.prefs.gpu";
    pub(crate) const PREFS_SF: &str = "cmd.prefs.sf";
    pub(crate) const PREFS_THERMAL: &str = "cmd.prefs.thermal";
    pub(crate) const PREFS_TRACE: &str = "cmd.prefs.trace";
    pub(crate) const PREFS_LOGD: &str = "cmd.prefs.logd";
    pub(crate) const PREFS_WRITE: &str = "cmd.prefs.write";

    // common: helpers shared by the daemon and the commands.
    pub(crate) const COMMON_FS: &str = "common.fs";
    pub(crate) const COMMON_PROP: &str = "common.prop";
}

/// Daemon logging macros. Every line is tagged `[daemon.*]`; pass the tag from
/// [`comp`] so a message names the module that produced it:
///
/// ```ignore
/// use crate::common::log::{comp, info, warn};
/// info!(comp::PROFILER, "profile [{}] -> [{}]", from, to);
/// ```
macro_rules! debug { ($component:expr, $($a:tt)*) => { $crate::common::log::log_write($crate::common::log::Lv::Debug, $component, &format!($($a)*)) }; }
macro_rules! info  { ($component:expr, $($a:tt)*) => { $crate::common::log::log_write($crate::common::log::Lv::Info,  $component, &format!($($a)*)) }; }
// different internal name: a plain `use warn` re-export clashes with the built-in `#[warn]` attribute
macro_rules! warn_internal { ($component:expr, $($a:tt)*) => { $crate::common::log::log_write($crate::common::log::Lv::Warn,  $component, &format!($($a)*)) }; }
macro_rules! error { ($component:expr, $($a:tt)*) => { $crate::common::log::log_write($crate::common::log::Lv::Error, $component, &format!($($a)*)) }; }
pub(crate) use debug;
pub(crate) use error;
pub(crate) use info;
pub(crate) use warn_internal as warn;

fn prio(lv: Lv) -> std::os::raw::c_int {
    match lv {
        Lv::Debug => android_ffi::ANDROID_LOG_DEBUG,
        Lv::Info => android_ffi::ANDROID_LOG_INFO,
        Lv::Warn => android_ffi::ANDROID_LOG_WARN,
        Lv::Error => android_ffi::ANDROID_LOG_ERROR,
    }
}

/// Emit one line to logcat, tagged with the component so `logcat -s AZenith`
/// reads the same shape everywhere. logcat already timestamps the line, so we
/// only prefix the component.
fn logcat(lv: Lv, component: &str, message: &str) {
    android_ffi::logcat(prio(lv), LOGCAT_TAG, &format!("[{component}] {message}"));
}

pub(crate) fn log_init() {
    let lv = match std::env::var("LIBBINDING_LOG").ok().as_deref() {
        Some("debug") => Lv::Debug,
        Some("info") => Lv::Info,
        Some("warn") => Lv::Warn,
        Some("error") => Lv::Error,
        _ if debugmode() => Lv::Debug,
        _ => Lv::Info,
    };
    LEVEL.store(lv as u8, Ordering::Relaxed);
}

/// `true` when DEBUG detail should be produced at all.
///
/// Driven by the `persist.azenith.debugmode` property alone, so flipping the
/// prop and restarting the service is the one way to turn verbose logging on.
pub(crate) fn debugmode() -> bool {
    crate::common::prop::getprop(DEBUGMODE_PROP) == "true"
}

/// Single funnel for every level, so the emitted shape stays identical
/// whichever module emitted the line.
pub(crate) fn log_write(lv: Lv, component: &str, msg: &str) {
    if (lv as u8) < LEVEL.load(Ordering::Relaxed) { return; }
    // DEBUG detail is opt-in, so a command that is invoked (or not) does not
    // have to check the property itself.
    if matches!(lv, Lv::Debug) && !debugmode() { return; }

    let component = if component.is_empty() { comp::DAEMON } else { component };
    logcat(lv, component, msg);
}
