// ---------------- AMS observer (IProcessObserver) + uid -> package ----------------
// Adapted from BinderMonitor.cpp & ProcessUtility.cpp in encore.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

use crate::daemon::codes::codes;
use crate::common::log::{comp, debug, error, info, warn};
use crate::daemon::ndk::{call, n, str_alloc, P, UNKNOWN};
use crate::daemon::state::{apply, is_app, st};

const FIRST_CALL: u32 = 1; // onProcessStarted (unused)

// ---------------- uid -> package via IPackageManager.getNameForUid ----------------
static PM: OnceLock<usize> = OnceLock::new();
static UID_CACHE: Mutex<BTreeMap<i32, String>> = Mutex::new(BTreeMap::new());

pub(crate) fn set_pm(b: P) { let _ = PM.set(b as usize); }

/// The `package` service handle, or null when it was never acquired or has since
/// died. Shared so the first-boot game scan (`daemon::apps`) does not open a
/// second handle to the same service.
pub(crate) fn pm() -> P {
    match PM.get() {
        Some(p) => *p as P,
        None => std::ptr::null_mut(),
    }
}

fn pkg_of_uid(uid: i32) -> Option<String> {
    if let Some(p) = UID_CACHE.lock().unwrap_or_else(|e| e.into_inner()).get(&uid) {
        return Some(p.clone());
    }
    let pm = *PM.get()? as P;
    let n = n();
    // the cache lock is NOT held during the synchronous call to system_server
    let o = call(pm, codes().pm_name, |i| unsafe { (n.AParcel_writeInt32)(i, uid); })?;
    let mut buf: Vec<u8> = Vec::new();
    let r = unsafe { (n.AParcel_readString)(o, &mut buf as *mut Vec<u8> as P, str_alloc) };
    unsafe { (n.AParcel_delete)(o); }
    if r != 0 { warn!(comp::OBSERVER, "readString getNameForUid failed, status={r}"); return None; }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let name = String::from_utf8_lossy(&buf[..end]).into_owned();
    if name.is_empty() { return None; }
    UID_CACHE.lock().unwrap_or_else(|e| e.into_inner()).insert(uid, name.clone());
    Some(name)
}

// ---------------- callbacks from AMS (oneway, must be fast) ----------------
pub(crate) unsafe extern "C" fn on_observer(_b: P, code: u32, inp: P, _o: P) -> i32 {
    let (n, c) = (n(), codes());
    let (mut pid, mut uid) = (-1i32, -1i32);
    unsafe {
        if code == c.fg_act {
            let mut fg = false;
            (n.AParcel_readInt32)(inp, &mut pid);
            (n.AParcel_readInt32)(inp, &mut uid);
            (n.AParcel_readBool)(inp, &mut fg);
            debug!(comp::OBSERVER, "onForegroundActivitiesChanged: pid={pid} uid={uid} fg={fg}");

            if fg && is_app(uid) {
                // Only "went to foreground" events change the active app.
                match pkg_of_uid(uid) {
                    Some(pkg) => {
                        let mut g = st();
                        let s = &mut *g;
                        if s.list.contains(&pkg) {
                            let same = matches!(&s.active, Some((p, _)) if *p == pkg);
                            if same {
                                if let Some((_, pids)) = &mut s.active { pids.insert(pid); }
                            } else {
                                info!(comp::OBSERVER, "gamelist app went to foreground: {pkg} (pid {pid})");
                                s.active = Some((pkg, BTreeSet::from([pid])));
                            }
                            apply(s, "foreground");
                        } else {
                            debug!(comp::OBSERVER, "fg pkg {pkg} not in gamelist, ignored");
                        }
                    }
                    None => debug!(comp::OBSERVER, "fg uid={uid}: package name not found"),
                }
            } else if !fg {
                // Going to background is intentionally ignored: the active app persists
                // until all of its processes die.
                let g = st();
                if matches!(&g.active, Some((_, pids)) if pids.contains(&pid)) {
                    debug!(comp::OBSERVER, "active app pid={pid} went to background, keeping profile 1");
                }
            }
            return 0;
        }
        if code == c.died {
            (n.AParcel_readInt32)(inp, &mut pid);
            (n.AParcel_readInt32)(inp, &mut uid);
            debug!(comp::OBSERVER, "onProcessDied: pid={pid} uid={uid}");
            let mut g = st();
            let s = &mut *g;
            let gone = match &mut s.active {
                Some((_, pids)) => pids.remove(&pid) && pids.is_empty(),
                None => false,
            };
            if gone {
                if let Some((pkg, _)) = s.active.take() {
                    info!(comp::OBSERVER, "active app died: {pkg} (pid {pid})");
                }
                apply(s, "active app process died");
            }
            return 0;
        }
        if code == c.fg_svc || code == FIRST_CALL { return 0; }
        warn!(comp::OBSERVER, "unknown transaction from observer: code={code}");
    }
    UNKNOWN
}

pub(crate) unsafe extern "C" fn on_am_died(_: P) {
    error!(comp::OBSERVER, "ActivityManager died (system_server restart), exiting so we get restarted");
    std::process::exit(1);
}
