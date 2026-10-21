//! Shared helpers for the `settings` command (the former `binprofiles` crate).
//!
//! Everything is re-exported here so the rest of the command can keep using
//! `use crate::cmd::settings::utils::*;`.

pub(crate) mod apps;
pub(crate) mod devfreq;
pub(crate) mod freq;
pub(crate) mod governor;
pub(crate) mod logging;
pub(crate) mod props;
pub(crate) mod scheme;
pub(crate) mod sysfs;

pub(crate) use self::apps::*;
pub(crate) use self::devfreq::*;
pub(crate) use self::freq::*;
pub(crate) use self::governor::*;
pub(crate) use self::logging::*;
pub(crate) use self::props::*;
pub(crate) use self::scheme::*;
pub(crate) use self::sysfs::*;
