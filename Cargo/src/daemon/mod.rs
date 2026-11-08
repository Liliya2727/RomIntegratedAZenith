//! The service binary itself: the binder daemon that keeps the active profile
//! in sync with the foreground app.
//!
//! Module layout follows `encore/jni/` in AZenith:
//!
//! | Module | Was |
//! |---|---|
//! | `ndk` | `BinderNDK.cpp` — `libbinder_ndk` via `dlsym` |
//! | `codes` | transaction-code resolver (embeds `binder_resolver.apk`) |
//! | `observer` | `BinderMonitor.cpp` / `ProcessUtility.cpp` — `IProcessObserver` |
//! | `applist` | `GameRegistry` / `InotifyWatcher` — `gamelist.txt` |
//! | `apps` | installed-package scan for the first-boot `gamelist.txt` |
//! | `status` | `BinderMonitor.cpp` — screen, saver, zen, battery |
//! | `state` | `state.toml`, the state store |
//! | `profiler` | `profile/*.c` — profile switching |
//!
//! The profiler engine and the other companion tools live in [`crate::cmd`] as
//! modules of this same binary; the daemon calls them directly instead of
//! spawning helper processes. Shared plumbing lives in [`crate::common`].

pub(crate) mod applist;
pub(crate) mod api;
pub(crate) mod apps;
pub(crate) mod codes;
pub(crate) mod ndk;
pub(crate) mod observer;
pub(crate) mod procs;
pub(crate) mod profiler;
pub(crate) mod state;
pub(crate) mod status;
