//
// Copyright (C) 2026-2027 Zexshia
//
// This file is part of AZenith Service.
//
// AZenith Service is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// AZenith Service is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with AZenith Service.  If not, see <https://www.gnu.org/licenses/>.
//

//! Profile engine: the functions that used to live in the standalone
//! `binprofiles` crate (`sys.azenith-profilesettings`).
//!
//! The daemon calls [`apply_mode`] directly from its worker thread; the
//! `profilesettings` subcommand remains as a thin CLI wrapper around the same
//! functions for manual/debug use.

mod chipsets;
mod profiles;
mod utils;

use std::path::Path;
use std::process::Command;
use profiles::*;
use utils::*;

/// CLI entry point for `sys.azenith-service profilesettings <action> ...`.
pub(crate) fn run(args: &[String]) {
    if !crate::cmd::verify_caller() {
        eprintln!("\x1b[31mError: This utility can only be called by sys.azenith-service\x1b[0m");
        std::process::exit(1);
    }
    run_action(args);
}

/// In-process entry used by the daemon profiler. `mode` is the same number the
/// CLI accepts: 0 = initialize, 1 = performance, 2 = balanced, 3 = eco.
pub(crate) fn apply_mode(mode: u8) {
    run_action(&[mode.to_string()]);
}

fn run_action(args: &[String]) {
    let Some(action) = args.first() else { return };

    match action.as_str() {
        "0" | "initialize" => initialize(),
        "1" | "performance_profile" => performance_profile(),
        "2" | "balanced_profile" => balanced_profile(),
        "3" | "eco_mode" => eco_mode(),
        "applyfreqbalance" => applyfreqbalance(),
        "applyfreqgame" => applyfreqgame(),
        _ => {
            if Path::new(action).exists() || action.contains('.') {
                let _ = Command::new(action)
                    .args(&args[1..])
                    .status();
            }
        }
    }
}
