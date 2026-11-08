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

//! System tweaks that used to live in the standalone `binutils` crate
//! (`sys.azenith-utilityconf`).
//!
//! The daemon calls [`trim`] directly during its initialize phase; the
//! `utilityconf` subcommand remains as a thin CLI wrapper for manual use.

mod utils;

use std::process::Command;
use utils::*;

/// In-process entry used by the daemon's initialize phase.
pub(crate) fn trim() {
    fstrim();
}

/// Run one utility function. `args` are the arguments *after* the subcommand
/// name (`sys.azenith-service utilityconf <function> ...`).
pub(crate) fn run(args: &[String]) {
    let Some(function) = args.first() else { return };
    let rest = &args[1..];

    match function.as_str() {
        "setsgov" => if let Some(v) = rest.first() { setsgov(v) },
        "setthermalcore" => if let Some(v) = rest.first() { setthermalcore(v) },
        "checkmalipath" => check_mali_path(),
        "FSTrim" => fstrim(),
        "enableDND" => enable_dnd(),
        "disableDND" => disable_dnd(),
        "setrefreshrates" => if let Some(v) = rest.first() { setrefreshrates(v) },
        "restartservice" => restartservice(),
        "setrender" => if let Some(v) = rest.first() { setrender(v) },
        _ => {
            let _ = Command::new(function).args(rest).status();
        }
    }
}
