//! Helpers shared by every part of the service binary.
//!
//! `common` is the only place where cross-cutting concerns live: the raw
//! Android C bindings ([`android_ffi`]), the log writer ([`log`]), filesystem
//! writes ([`fs`]), system properties ([`prop`]), the saved CPU governor
//! ([`cpugov`]) and the occasional forked shell ([`shell`]). The binder daemon
//! (`daemon/`), the companion commands (`cmd/`) and `main.rs` are all built on
//! top of it — nothing in here may depend on them.

pub(crate) mod android_ffi;
pub(crate) mod cpugov;
pub(crate) mod fs;
pub(crate) mod log;
pub(crate) mod paths;
pub(crate) mod prop;
pub(crate) mod shell;
