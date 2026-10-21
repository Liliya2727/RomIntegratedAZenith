// ---------------- installed packages, straight from the package manager ----------------
//
// The first-boot gamelist scan asks the *package manager* what is installed
// rather than guessing from `/data/app` or from running processes: it is the one
// source that knows about every package, including system ones and apps on an
// adopted storage volume.
//
// The question is `IPackageManager.getAllPackages()`, which answers with a plain
// `List<String>`. That is the one shape this code can read with the symbols
// libbinder_ndk actually exports: `getInstalledPackages`/`getInstalledApplications`
// answer with a `ParceledListSlice<PackageInfo>`, and reading those would mean
// walking a sixty-field record per package using `writeString8` fields for which
// the NDK has no reader (its only string reader is the `writeString`/UTF-16 one),
// so a name list is the only reply that can be decoded without a per-release
// hand-written record layout.
//
// The reply is `int32 N` followed by `N` UTF-16 strings — `writeStringList` in
// `Parcel`, which is exactly what `AParcel_readString` reads.

use std::collections::BTreeSet;

use crate::common::log::{comp, debug, warn};
use crate::daemon::ndk::{call, n, str_alloc, P};
use crate::daemon::observer::pm;
use crate::daemon::status::service_retry;

/// How many packages must come back before a reply is believed. No real device
/// has fewer, and the count doubles as a sanity check: a walk that decoded the
/// wrong shape returns junk names, which [`looks_like_package`] drops.
const MIN_PACKAGES: usize = 8;

/// Package names the scan must never put in the gamelist: the daemon and its
/// companion UI are not games, and a profile applied to them is wasted work.
const NEVER_GAME: [&str; 1] = ["zx.azenith"];

/// The resolver query that yields the transaction code.
const QUERY: &str = "android.content.pm.IPackageManager.Stub::TRANSACTION_getAllPackages";

/// Every installed package name, or `None` if the package manager could not be
/// asked at all. Callers retry later rather than treating `None` as "no games".
pub(crate) fn installed_packages() -> Option<BTreeSet<String>> {
    let pm = pm();
    if pm.is_null() {
        warn!(comp::APPLIST, "package manager not available, cannot scan installed packages");
        return None;
    }
    let pm_service = service_retry(c"package", c"android.content.pm.IPackageManager");
    if pm_service.is_null() {
        warn!(comp::APPLIST, "package service not available, cannot scan installed packages");
        return None;
    }

    let code = crate::daemon::codes::resolve_code(QUERY).unwrap_or(0);
    if code == 0 {
        warn!(comp::APPLIST, "getAllPackages: transaction code not found, cannot scan installed packages");
        return None;
    }
    let pkgs = query(pm_service, code)?;
    if pkgs.len() < MIN_PACKAGES {
        warn!(comp::APPLIST, "getAllPackages returned only {} usable names", pkgs.len());
        return None;
    }
    debug!(comp::APPLIST, "getAllPackages returned {} package names", pkgs.len());
    Some(pkgs)
}

/// One `getAllPackages` transaction. `None` when the transaction itself failed;
/// `Some` (possibly empty) once a reply came back.
fn query(pm: P, code: u32) -> Option<BTreeSet<String>> {
    let n = n();
    // `getAllPackages()` takes no arguments.
    let out = call(pm, code, |_| {})?;

    let mut pkgs = BTreeSet::new();

    // `writeStringList`: a signed count, `-1` for null.
    let mut count = 0i32;
    if unsafe { (n.AParcel_readInt32)(out, &mut count) } != 0 || count <= 0 {
        unsafe { (n.AParcel_delete)(out); }
        return Some(pkgs);
    }

    for _ in 0..count.min(100_000) {
        // Each element is a `writeString` (UTF-16) value, so the NDK string
        // reader is the right decoder here.
        let mut buf: Vec<u8> = Vec::new();
        if unsafe { (n.AParcel_readString)(out, &mut buf as *mut Vec<u8> as P, str_alloc) } != 0 {
            break;
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let name = String::from_utf8_lossy(&buf[..end]).into_owned();
        if looks_like_package(&name) {
            pkgs.insert(name);
        }
    }

    unsafe { (n.AParcel_delete)(out); }
    Some(pkgs)
}

/// A package name is a dotted identifier with an optional `:process` suffix.
/// This keeps junk out of the list when a reply was decoded from the wrong
/// layout, and drops the daemon's own companion app.
pub(crate) fn looks_like_package(name: &str) -> bool {
    let name = name.split(':').next().unwrap_or("");
    if !name.contains('.') || name.is_empty() || name.len() > 255 {
        return false;
    }
    if !name.split('.').all(|seg| {
        !seg.is_empty()
            && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && seg.starts_with(|c: char| c.is_ascii_alphabetic())
    }) {
        return false;
    }
    !NEVER_GAME.iter().any(|skip| name == *skip || name.starts_with(&format!("{skip}.")))
}
