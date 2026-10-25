// ---------------- AMS observer (IProcessObserver) + uid -> package ----------------
// Adapted from sysdaemon in azenith (observer.rs).
use std::collections::BTreeMap;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;

use crate::common::log::{comp, debug, error, info, warn};
use crate::daemon::codes::codes;
use crate::daemon::ndk::{call, n, str_alloc, P, UNKNOWN};
use crate::daemon::state;

const FIRST_CALL: u32 = 1; // onProcessStarted

pub(crate) enum Event {
    Foreground { pid: i32, uid: i32 },
    Started { pid: i32, uid: i32, pkg: String },
    Died { pid: i32 },
}

static TX: OnceLock<Sender<Event>> = OnceLock::new();

fn send(ev: Event) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(ev);
    }
}

pub(crate) fn start() {
    let (tx, rx) = mpsc::channel::<Event>();
    if TX.set(tx).is_err() { return; }
    thread::spawn(move || {
        while let Ok(ev) = rx.recv() {
            match ev {
                Event::Foreground { pid, uid } => {
                    if !state::is_app(uid) { continue; }
                    let pkg = state::pkg_of_pid(pid)
                        .or_else(|| crate::daemon::procs::pkg_of(pid))
                        .or_else(|| pkg_of_uid(uid));
                    match pkg {
                        Some(pkg) => state::set_focused(&pkg, pid, uid, "foreground"),
                        None => debug!(comp::OBSERVER, "foreground: pid={pid} uid={uid}: package name not found"),
                    }
                }
                Event::Started { pid, uid, pkg } => {
                    if !state::is_app(uid) { continue; }
                    let pkg = if pkg.is_empty() {
                        match pkg_of_uid(uid) {
                            Some(p) => p,
                            None => continue,
                        }
                    } else {
                        pkg
                    };
                    debug!(comp::OBSERVER, "process started: pid={pid} uid={uid} pkg={pkg}");
                    state::add_proc(pid, &pkg, uid);
                }
                Event::Died { pid } => {
                    debug!(comp::OBSERVER, "onProcessDied: pid={pid}");
                    state::remove_proc(pid, "process died");
                }
            }
        }
        info!(comp::OBSERVER, "event worker stopped");
    });
}

// ---------------- uid -> package via IPackageManager.getNameForUid ----------------
static PM: OnceLock<usize> = OnceLock::new();
static UID_CACHE: Mutex<BTreeMap<i32, String>> = Mutex::new(BTreeMap::new());

pub(crate) fn set_pm(b: P) { let _ = PM.set(b as usize); }

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
    let o = call(pm, codes().pm_name, |i| unsafe { (n.AParcel_writeInt32)(i, uid); })?;
    let mut buf: Vec<u8> = Vec::new();
    let r = unsafe { (n.AParcel_readString)(o, &mut buf as *mut Vec<u8> as P, str_alloc) };
    unsafe { (n.AParcel_delete)(o); }
    if r != 0 { return None; }
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
            if fg {
                send(Event::Foreground { pid, uid });
            }
            return 0;
        }
        if code == c.proc_started || code == FIRST_CALL {
            (n.AParcel_readInt32)(inp, &mut pid);
            (n.AParcel_readInt32)(inp, &mut uid);
            let mut buf: Vec<u8> = Vec::new();
            let r = (n.AParcel_readString)(inp, &mut buf as *mut Vec<u8> as P, str_alloc);
            let pkg = if r == 0 {
                let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                String::from_utf8_lossy(&buf[..end]).into_owned()
            } else {
                String::new()
            };
            let pkg = crate::daemon::procs::base_pkg(&pkg).unwrap_or_default();
            if !pkg.is_empty() || state::is_app(uid) {
                send(Event::Started { pid, uid, pkg });
            }
            return 0;
        }
        if code == c.died {
            (n.AParcel_readInt32)(inp, &mut pid);
            (n.AParcel_readInt32)(inp, &mut uid);
            send(Event::Died { pid });
            return 0;
        }
        if code == c.fg_svc { return 0; }
        warn!(comp::OBSERVER, "unknown transaction from observer: code={code}");
    }
    UNKNOWN
}

pub(crate) unsafe extern "C" fn on_am_died(_: P) {
    error!(comp::OBSERVER, "ActivityManager died (system_server restart), exiting so we get restarted");
    std::process::exit(1);
}
