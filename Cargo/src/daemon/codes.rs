// ---------------- transaction codes (cache + resolver embedded) ----------------
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::common::log::{comp, debug, error, info, warn};

// Digenerate dari Config.toml (lihat Cargo/build.rs dan crate::common::paths).
use crate::common::paths::BINDER_RESOLVER_APK as APK_PATH;
const RESOLVER_APK: &[u8] = include_bytes!("../../binder_resolver.apk");

pub(crate) struct Codes {
    pub(crate) reg: u32, pub(crate) fg_act: u32, pub(crate) fg_svc: u32,
    pub(crate) died: u32, pub(crate) pm_name: u32,
    pub(crate) is_interactive: u32, pub(crate) is_saver: u32, pub(crate) get_zen: u32,
    pub(crate) disp_reg: u32, pub(crate) bat_prop: u32,
    pub(crate) proc_started: u32,
}
static CODES: OnceLock<Codes> = OnceLock::new();
pub(crate) fn codes() -> &'static Codes { CODES.get().unwrap() }
pub(crate) fn set_codes(c: Codes) { let _ = CODES.set(c); }

const Q: [&str; 11] = [
    "android.app.IActivityManager.Stub::TRANSACTION_registerProcessObserver",
    "android.app.IProcessObserver.Stub::TRANSACTION_onForegroundActivitiesChanged",
    "android.app.IProcessObserver.Stub::TRANSACTION_onForegroundServicesChanged",
    "android.app.IProcessObserver.Stub::TRANSACTION_onProcessDied",
    "android.content.pm.IPackageManager.Stub::TRANSACTION_getNameForUid",
    "android.os.IPowerManager.Stub::TRANSACTION_isInteractive",
    "android.os.IPowerManager.Stub::TRANSACTION_isPowerSaveMode",
    "android.app.INotificationManager.Stub::TRANSACTION_getZenMode",
    "android.hardware.display.IDisplayManager.Stub::TRANSACTION_registerCallback",
    "android.os.IBatteryPropertiesRegistrar.Stub::TRANSACTION_getProperty",
    "android.app.IProcessObserver.Stub::TRANSACTION_onProcessStarted",
];

fn prop(k: &str) -> String {
    Command::new("getprop").arg(k).output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

// Entry point of the binder_resolver APK. The apk built from ./binder_resolver uses
// zx.binder.MainKt, the older prebuilt (encore) uses com.rem01gaming.binderresolver.MainKt.
const ENTRY_CLASSES: [&str; 2] = ["zx.binder.MainKt", "com.rem01gaming.binderresolver.MainKt"];

fn run_resolver(apk: &str, entry: &str) -> String {
    run_resolver_entry(apk, entry, &Q)
}

/// Run the embedded resolver APK for one non-code question and return its
/// stdout. The APK is staged in `BINDER_RESOLVER_APK` and removed afterwards,
/// just like [`load_codes`]. `None` when no entry point produced any output.
pub(crate) fn run_query(question: &str) -> Option<String> {
    let _ = fs::remove_file(APK_PATH);
    if let Err(e) = fs::write(APK_PATH, RESOLVER_APK) {
        error!(comp::CODES, "failed to write {APK_PATH}: {e}");
        return None;
    }
    if let Err(e) = fs::set_permissions(APK_PATH, fs::Permissions::from_mode(0o444)) {
        error!(comp::CODES, "chmod failed for {APK_PATH}: {e}");
        return None;
    }

    let mut result = None;
    for entry in ENTRY_CLASSES {
        let out = run_resolver_entry(APK_PATH, entry, &[question]);
        if !out.trim().is_empty() {
            debug!(comp::CODES, "resolver entry point: {entry}");
            result = Some(out);
            break;
        }
        warn!(comp::CODES, "resolver produced no usable output with {entry}, trying the next entry point");
    }
    let _ = fs::remove_file(APK_PATH);
    result
}

fn run_resolver_entry(apk: &str, entry: &str, questions: &[&str]) -> String {
    let Ok(mut ch) = Command::new("/system/bin/app_process")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD")
        .env("ANDROID_ROOT", "/system").env("ANDROID_DATA", "/data")
        .env("ANDROID_RUNTIME_ROOT", "/apex/com.android.runtime")
        .env("ANDROID_ART_ROOT", "/apex/com.android.art")
        .env("ANDROID_I18N_ROOT", "/apex/com.android.i18n")
        .env("ANDROID_TZDATA_ROOT", "/apex/com.android.tzdata")
        .arg(format!("-Djava.class.path={apk}")).arg("/")
        .arg(entry)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
        .spawn() else {
            error!(comp::CODES, "failed to spawn app_process");
            return String::new();
        };
    if let Some(mut si) = ch.stdin.take() {
        let _ = si.write_all(questions.join("\n").as_bytes());
        let _ = si.write_all(b"\n");
    }
    ch.wait_with_output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

fn parse_codes(text: &str) -> Option<Codes> {
    let get = |q: &str| text.lines().find_map(|l| {
        let (k, v) = l.rsplit_once(' ')?;
        if k == q { v.parse::<u32>().ok() } else { None }
    }).unwrap_or(0);
    let proc_started = get(Q[10]);
    let c = Codes {
        reg: get(Q[0]), fg_act: get(Q[1]), fg_svc: get(Q[2]),
        died: get(Q[3]), pm_name: get(Q[4]),
        is_interactive: get(Q[5]), is_saver: get(Q[6]), get_zen: get(Q[7]),
        disp_reg: get(Q[8]),
        bat_prop: get(Q[9]),
        proc_started: if proc_started != 0 { proc_started } else { 1 },
    };
    // bat_prop is optional (the battery field becomes "unknown" if it is missing)
    let req = [c.reg, c.fg_act, c.died, c.pm_name,
               c.is_interactive, c.is_saver, c.get_zen, c.disp_reg];
    req.iter().all(|&v| v != 0).then_some(c)
}

fn log_codes(c: &Codes) {
    info!(comp::CODES, "transaction codes: registerProcessObserver={} fgActivities={} fgServices={} processDied={} onProcessStarted={} getNameForUid={}",
          c.reg, c.fg_act, c.fg_svc, c.died, c.proc_started, c.pm_name);
    info!(comp::CODES, "transaction codes: isInteractive={} isPowerSaveMode={} getZenMode={} registerDisplayCallback={} getProperty={}",
          c.is_interactive, c.is_saver, c.get_zen, c.disp_reg, c.bat_prop);
}

pub(crate) fn load_codes() -> Option<Codes> {
    let sdk = prop("ro.build.version.sdk");
    let inc: String = prop("ro.build.version.incremental").chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let cache = format!("{}.{sdk}.{inc}", crate::common::paths::CODES_CACHE);
    info!(comp::CODES, "device: sdk={sdk} incremental={inc}");

    if let Some(c) = fs::read_to_string(&cache).ok().and_then(|t| parse_codes(&t)) {
        info!(comp::CODES, "transaction codes from cache: {cache}");
        log_codes(&c);
        return Some(c);
    }

    info!(comp::CODES, "cache missing/invalid, running resolver ({} byte APK)", RESOLVER_APK.len());
    let _ = fs::remove_file(APK_PATH);
    if let Err(e) = fs::write(APK_PATH, RESOLVER_APK) {
        error!(comp::CODES, "failed to write {APK_PATH}: {e}");
        return None;
    }
    if let Err(e) = fs::set_permissions(APK_PATH, fs::Permissions::from_mode(0o444)) {
        error!(comp::CODES, "chmod failed for {APK_PATH}: {e}");
        return None;
    }
    let t0 = std::time::Instant::now();
    let mut out = String::new();
    for entry in ENTRY_CLASSES {
        out = run_resolver(APK_PATH, entry);
        if parse_codes(&out).is_some() {
            debug!(comp::CODES, "resolver entry point: {entry}");
            break;
        }
        warn!(comp::CODES, "resolver produced no usable output with {entry}, trying the next entry point");
    }
    let _ = fs::remove_file(APK_PATH);
    debug!(comp::CODES, "resolver finished in {} ms, output:\n{}", t0.elapsed().as_millis(), out.trim_end());

    let Some(c) = parse_codes(&out) else {
        error!(comp::CODES, "resolver output incomplete, required codes not found");
        return None;
    };
    match fs::write(&cache, &out) {
        Ok(()) => info!(comp::CODES, "transaction codes saved to cache: {cache}"),
        Err(e) => warn!(comp::CODES, "failed to save cache {cache}: {e}"),
    }
    log_codes(&c);
    if c.bat_prop == 0 {
        warn!(comp::CODES, "getProperty code not found, battery field not monitored");
    }
    Some(c)
}
