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

//! Preference tweaks that used to live in the standalone `binpreferenced`
//! crate (`sys.azenith-preferencedtweaks`).
//!
//! The daemon calls [`apply`] directly during its initialize phase; the
//! `preferencedtweaks` subcommand remains as a thin CLI wrapper for manual use.

mod tweaks;
mod utils;

use std::path::Path;
use std::process::Command;
use tweaks::*;
use utils::*;

/// CLI entry point for
/// `sys.azenith-service preferencedtweaks [action] ...`.
pub(crate) fn run(args: &[String]) {
    if !crate::cmd::verify_caller() {
        eprintln!("\x1b[31mError: This utility can only be called by sys.azenith-service\x1b[0m");
        std::process::exit(1);
    }
    run_action(args);
}

/// In-process entry used by the daemon's initialize phase.
pub(crate) fn apply() {
    run_action(&[]);
}

fn run_action(args: &[String]) {
    init_debugmode();

    match args.first().map(String::as_str) {
        Some("apply") | Some("prefs") | Some("preferenced") => { prefsettings(); }
        Some(action) if Path::new(action).exists() || action.contains('.') => {
            let _ = Command::new(action)
                .args(&args[1..])
                .status();
        }
        Some(cmd) => {
            if !cmd.is_empty() {
                eprintln!("unknown preferencedtweaks action: {cmd} (running the full set)");
            }
            prefsettings();
        }
        None => { prefsettings(); }
    };

    let _ = Command::new("sync").status();
}
