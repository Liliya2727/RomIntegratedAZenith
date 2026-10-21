// ---------------- system status (screen, battery saver, zen, battery level & charging) ----------------
// Adapted from BinderMonitor.cpp in encore, but with no shell forking at all.
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::{thread, time::Duration};

use crate::daemon::codes::codes;
use crate::common::log::{comp, error, info, warn};
use crate::daemon::ndk::{call, call_i32, class, n, service, P};
use crate::daemon::state::sys_update;

// Binder query cycle interval for zen/saver/battery (pure transact, no fork).
// 500ms = same as pollPowerSave() in encore. 4 transacts per cycle (8/sec),
// each transact only tens of microseconds. Android has no push event for these
// three values; see the note in watch_status().
const STATUS_POLL: Duration = Duration::from_millis(500);
const PROP_CAPACITY: i32 = 4; // BatteryManager.BATTERY_PROPERTY_CAPACITY
const PROP_STATUS: i32 = 6;   // BatteryManager.BATTERY_PROPERTY_STATUS

static SVC_POWER: OnceLock<usize> = OnceLock::new();     // android.os.IPowerManager
static SVC_NOTIFY: OnceLock<usize> = OnceLock::new();    // android.app.INotificationManager
static SVC_BATPROP: OnceLock<usize> = OnceLock::new();   // android.os.IBatteryPropertiesRegistrar
static DISPLAY_CB: OnceLock<usize> = OnceLock::new();    // IDisplayManagerCallback (kept alive)

// IPowerManager.isInteractive -> screen on/off. Called from the display callback & at startup.
fn refresh_screen(why: &str) {
    let Some(&p) = SVC_POWER.get() else { return };
    match call_i32(p as P, codes().is_interactive) {
        Some(v) => sys_update(why, |s| s.screen = if v != 0 { "on" } else { "off" }),
        None => warn!(comp::STATUS, "isInteractive failed ({why})"),
    }
}

// IPowerManager.isPowerSaveMode -> battery saver on/off
fn refresh_saver(why: &str) {
    let Some(&p) = SVC_POWER.get() else { return };
    match call_i32(p as P, codes().is_saver) {
        Some(v) => sys_update(why, |s| s.saver = if v != 0 { "on" } else { "off" }),
        None => warn!(comp::STATUS, "isPowerSaveMode failed ({why})"),
    }
}

// INotificationManager.getZenMode -> 0=off, 1=priority, 2=none, 3=alarms
fn refresh_zen(why: &str) {
    let Some(&p) = SVC_NOTIFY.get() else { return };
    match call_i32(p as P, codes().get_zen) {
        Some(v) => sys_update(why, |s| s.zen = match v {
            0 => "off", 1 => "priority", 2 => "none", 3 => "alarms", _ => "unknown",
        }),
        None => warn!(comp::STATUS, "getZenMode failed ({why})"),
    }
}

/// Look a service up with more patience than [`service`] has (1s).
///
/// The first-boot game scan runs from `initialize`, which happens early enough
/// that a service it does not depend on can still be starting; giving up on the
/// first second would silently leave the gamelist empty until the next boot.
pub(crate) fn service_retry(name: &std::ffi::CStr, desc: &std::ffi::CStr) -> P {
    crate::daemon::ndk::service_try(name, desc, 50)
}

// BatteryPropertiesRegistrar.getProperty(id).// Reply after call() strips the status header: result(int32) + [presence(int32)] + int64 + string8.
// Newer AIDL writes presence before the parcelable out, older AIDL does not -> try both.
fn bat_prop(id: i32) -> Option<i32> {
    let b = *SVC_BATPROP.get()? as P;
    let o = call(b, codes().bat_prop, |i| unsafe {
        (n().AParcel_writeInt32)(i, id);
    })?;
    let nd = n();
    let mut w = [0i32; 5];
    let ok = unsafe { (0..5).all(|k| (nd.AParcel_readInt32)(o, &mut w[k]) == 0) };
    unsafe { (nd.AParcel_delete)(o); }
    if !ok || w[0] != 0 { return None; } // w[0] = getProperty result, 0 = OK
    // [1]=presence [2]=lo [3]=hi [4]=string8 length (-1 = null)
    if w[1] == 1 && w[3] == 0 { return Some(w[2]); }
    // old layout without presence: [1]=lo [2]=hi [3]=string8 length
    if w[2] == 0 { return Some(w[1]); }
    None
}

// getProperty(BATTERY_PROPERTY_CAPACITY) -> 0..100
fn battery_level() -> Option<i32> {
    bat_prop(PROP_CAPACITY).filter(|v| (0..=100).contains(v))
}

// getProperty(BATTERY_PROPERTY_STATUS) -> BatteryManager.BATTERY_STATUS_*
// 2=CHARGING, 5=FULL -> "charging"; 3=DISCHARGING, 4=NOT_CHARGING -> "discharging".
// IMPORTANT NOTE: do not use IBatteryStats.isCharging() for this. It is not a plug
// state, but BatteryStatsImpl's ACTION_CHARGING broadcast heuristic
// (setChargingLocked: only true if the level ROSE or >=90%) -> returns false even when
// the charger is plugged in. Proven live: "service call batterystats 17" = 0 while
// sysfs status=Charging / dumpsys status:2, whereas getProperty(6) = 2 (CHARGING).
fn battery_charging() -> Option<&'static str> {
    match bat_prop(PROP_STATUS)? {
        2 | 5 => Some("charging"),
        3 | 4 => Some("discharging"),
        _ => None, // 0/1 = unknown, keep the old value
    }
}

fn refresh_battery(why: &str) {
    let lvl = battery_level();
    let chg = battery_charging();
    if lvl.is_none() && chg.is_none() { return; } // service missing, do not overwrite with unknown
    sys_update(why, |s| {
        if let Some(l) = lvl { s.level = l; }
        if let Some(c) = chg { s.charge = c; }
    });
}

// ---------------- binder callbacks ----------------
// IDisplayManagerCallback.onDisplayEvent: oneway, packet contents do not matter (displayId, event).
// It is just a "display changed" marker -> re-query isInteractive over binder.
unsafe extern "C" fn on_display(_b: P, _code: u32, _inp: P, _o: P) -> i32 {
    refresh_screen("display event");
    0
}

// Status query cycle: battery saver, zen_mode, battery — all pure binder
// transacts (2-4 calls per cycle), with no process forking whatsoever.
// Why not events: IContentService.registerContentObserver calls
// ActivityManagerInternal.checkContentProviderAccess(), which REQUIRES a ProcessRecord
// for the caller PID (mPidsSelfLocked). A forked native process (this root daemon /
// app_process) never has a ProcessRecord — only Zygote apps do — so the server rejects
// it with EX_SECURITY 'Failed to find PID' (verified live on Android 16).
// IPowerManager & INotificationManager themselves have no register-listener method at
// all for power-save / zen, so this light binder polling is the only way to get these
// three values. (encore does the same — pollPowerSave() every 500ms — we do a full
// cycle of 4 values/500ms.)
fn watch_status() {
    thread::spawn(|| loop {
        refresh_saver("status cycle");
        refresh_zen("status cycle");
        refresh_battery("status cycle");
        crate::daemon::profiler::tick(); // drops performance when the screen-off grace period expires
        thread::sleep(STATUS_POLL);
    });
}

pub(crate) fn init_status() {
    let c = codes();

    let power = service(c"power", c"android.os.IPowerManager");
    if power.is_null() { warn!(comp::STATUS, "power service missing, screen/battery saver not monitored"); }
    else { let _ = SVC_POWER.set(power as usize); }

    let notif = service(c"notification", c"android.app.INotificationManager");
    if notif.is_null() { warn!(comp::STATUS, "notification service missing, zen_mode not monitored"); }
    else { let _ = SVC_NOTIFY.set(notif as usize); }

    // optional: if missing, the battery field stays "unknown"
    let batp = service_retry(c"batteryproperties", c"android.os.IBatteryPropertiesRegistrar");
    if batp.is_null() { warn!(comp::STATUS, "batteryproperties missing, battery level & charging not monitored"); }
    else { let _ = SVC_BATPROP.set(batp as usize); }

    // ---------------- status listener setup ----------------
    // Note: a ContentObserver for settings/global (zen_mode, low_power) cannot be used
    // from a native process: registerContentObserver goes through
    // checkContentProviderAccess(), which requires a ProcessRecord for the caller PID.
    // zen/saver values are read via binder queries in watch_status().

    // display: screen on/off events (callback -> re-query isInteractive)
    let disp = service(c"display", c"android.hardware.display.IDisplayManager");
    if disp.is_null() { warn!(comp::STATUS, "display service missing, screen not monitored"); }
    else {
        let cb = unsafe { (n().AIBinder_new)(class(c"android.hardware.display.IDisplayManagerCallback", on_display), null_mut()) };
        if cb.is_null() { error!(comp::STATUS, "failed to create IDisplayManagerCallback"); }
        else if let Some(r) = call(disp, c.disp_reg, |i| unsafe { (n().AParcel_writeStrongBinder)(i, cb); }) {
            unsafe { (n().AParcel_delete)(r); }
            let _ = DISPLAY_CB.set(cb as usize);
            info!(comp::STATUS, "IDisplayManagerCallback registered with display");
        } else {
            error!(comp::STATUS, "display registerCallback denied, screen not monitored");
        }
    }

    // initial values, then start the status query cycle
    refresh_screen("startup");
    refresh_saver("startup");
    refresh_zen("startup");
    refresh_battery("startup");
    watch_status();
}
