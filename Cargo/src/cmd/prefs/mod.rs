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

//! `sys.azenith-service preferencedtweaks [apply|prefs|preferenced]` — the
//! preference tweaks that used to live in the standalone `binpreferenced`
//! crate (`sys.azenith-preferencedtweaks`).

mod tweaks;
mod utils;

use std::path::Path;
use std::process::Command;
use tweaks::*;
use utils::*;

/// Run the tweaks. `args` are the arguments *after* the subcommand name
/// (`sys.azenith-service preferencedtweaks [action] ...`).
pub(crate) fn run(args: &[String]) {
    if !crate::cmd::verify_caller() {
        eprintln!("\x1b[31mError: This utility can only be called by sys.azenith-service\x1b[0m");
        std::process::exit(1);
    }

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
