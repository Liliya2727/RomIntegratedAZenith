//! CPU governor the daemon saved before a profile overwrote it.
//!
//! Per-profile governor resolution works like this now:
//!
//! * `performance` / `eco` — if `custom_performance_cpu_gov` /
//!   `custom_powersave_cpu_gov` is set it wins. Otherwise the governor currently
//!   on cpu0 is saved (see [`save`]) and the mode's own governor (`performance` /
//!   `powersave`) is applied.
//! * `balanced` — `custom_default_cpu_gov` wins when set; otherwise the saved
//!   governor is restored. With nothing ever saved (first apply after boot) the
//!   kernel baseline is left untouched.
//!
//! The saved value is a plain daemon flag, not a property or a file: it lives
//! for the daemon's lifetime and is lost on restart, after which the first
//! `balanced` apply simply adopts whatever governor the kernel came up with.

use std::sync::Mutex;

/// The governor to restore on the next `balanced` apply.
#[derive(Default)]
pub(crate) struct CpuGov {
    pub(crate) saved: Option<String>,
}

static STATE: Mutex<CpuGov> = Mutex::new(CpuGov { saved: None });

fn st() -> std::sync::MutexGuard<'static, CpuGov> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The governor to go back to, or `""` when nothing has been saved yet.
pub(crate) fn saved() -> String {
    st().saved.clone().unwrap_or_default()
}

/// Remember `gov` as the governor to restore, overwriting any previous value.
///
/// Called on every entry into `performance`/`eco`, so what is held is always the
/// governor that was in place *just before this mode switch*. Every mode change
/// therefore re-anchors `balanced` to the governor the kernel was on between two
/// modes — not to whatever the first switch after boot saw.
pub(crate) fn save(gov: &str) {
    st().saved = Some(gov.to_string());
}
