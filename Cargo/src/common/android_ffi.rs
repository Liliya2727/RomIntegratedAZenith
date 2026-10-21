//! Android C bindings shared by the whole service binary.
//!
//! Declaration-only, the same shape as `thermalcore/src/android_ffi.rs`. Three
//! families live here, and nothing else in the tree declares them directly:
//!
//! * **logcat** (`__android_log_print`) — used by [`crate::common::log`] to
//!   mirror every line it writes.
//! * **properties** (`__system_property_get` / `__system_property_set`) — the
//!   same calls the `getprop`/`setprop` *binaries* make, but without the fork.
//!   Note `__system_property_set` cannot delete a property or override `ro.*`,
//!   which is why `resetprop` still spawns a process.
//! * **flock** (`flock`) — the exclusive lock that serializes concurrent log
//!   writers, matching `write2file(..., use_flock = true, ...)` in the C daemon.
//!
//! Logging *policy* (paths, rotation, line shape) stays in
//! [`crate::common::log`]; this module only exposes the raw calls.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

/// Priorities from `<android/log.h>`.
pub(crate) const ANDROID_LOG_DEBUG: c_int = 3;
pub(crate) const ANDROID_LOG_INFO: c_int = 4;
pub(crate) const ANDROID_LOG_WARN: c_int = 5;
pub(crate) const ANDROID_LOG_ERROR: c_int = 6;

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

/// Mirror one line to logcat. The body goes through `%s` so a literal `%` inside
/// a message can never be treated as a format specifier.
pub(crate) fn logcat(prio: c_int, tag: &str, message: &str) {
    let Ok(c_tag) = CString::new(tag.replace('\0', "")) else { return };
    let Ok(c_msg) = CString::new(message.replace('\0', "")) else { return };
    let fmt = c"%s";
    unsafe {
        (__android_log_print)(prio, c_tag.as_ptr(), fmt.as_ptr(), c_msg.as_ptr());
    }
}

/// Property value buffer limit from `<sys/system_properties.h>`.
const PROP_VALUE_MAX: usize = 92;

/// Read a system property through bionic (`__system_property_get` — the same
/// call the `getprop` binary makes). `None` when the property is unset or
/// empty, matching what `getprop <key>` prints for a missing property.
pub(crate) fn prop_get(name: &str) -> Option<String> {
    let c_name = CString::new(name).ok()?;
    let mut buf = [0 as c_char; PROP_VALUE_MAX];
    let len = unsafe { libc::__system_property_get(c_name.as_ptr(), buf.as_mut_ptr()) };
    if len <= 0 {
        return None;
    }
    Some(unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned())
}

/// Set a system property through bionic (`__system_property_set` — the same
/// call the `setprop` binary makes). Returns `true` on success.
pub(crate) fn prop_set(name: &str, value: &str) -> bool {
    let Ok(c_name) = CString::new(name) else { return false };
    let Ok(c_value) = CString::new(value) else { return false };
    unsafe { libc::__system_property_set(c_name.as_ptr(), c_value.as_ptr()) == 0 }
}
