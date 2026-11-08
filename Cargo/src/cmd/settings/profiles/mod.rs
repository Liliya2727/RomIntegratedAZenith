//! Profiler: one init phase plus three active profiles.
//!
//! | Function | Mode | CLI name |
//! |---|---|---|
//! | [`initialize`] | `0` | `initialize` |
//! | [`performance_profile`] | `1` | `performance_profile` |
//! | [`balanced_profile`] | `2` | `balanced_profile` |
//! | [`eco_mode`] | `3` | `eco_mode` |
//!
//! The daemon calls these directly through
//! [`apply_mode`](super::apply_mode); the names in the last column are accepted
//! by the `profilesettings` CLI wrapper.
//!
//! **Taxonomy**: all three active profiles use the exact same step order
//! (see the [`common`] module docs); only `initialize` is a different phase.

mod balanced;
mod common;
mod eco;
mod initialize;
mod performance;

pub use balanced::balanced_profile;
pub use eco::eco_mode;
pub use initialize::initialize;
pub use performance::performance_profile;
