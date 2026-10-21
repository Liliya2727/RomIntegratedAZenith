//
// Copyright (C) 2026-2027 Zexshia
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

//! `sys.azenith-service utilityconf <function>` — the system tweaks that used
//! to live in the standalone `binutils` crate (`sys.azenith-utilityconf`).

mod utils;

use std::process::Command;
use utils::*;

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
