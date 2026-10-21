//! Background application handling.

use std::collections::HashSet;
use std::process::Command;

use crate::common::log::comp;
use crate::cmd::settings::utils::{log_debug, log_info};

/// Force-stop every package that is not visible in any window.
pub(crate) fn clear_background_apps() {
    let output = Command::new("dumpsys").args(["window", "displays"]).output();
    let stdout = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => return,
    };

    let mut visible_pkgs = HashSet::new();
    let mut invisible_pkgs = HashSet::new();

    // Read the output line by line (same as grep "Task{")
    for line in stdout.lines() {
        if line.contains("Task{") {
            // Replicates the sed logic: 's/.*A=[0-9]*:\([^ ]*\).*/\1/p'
            if let Some(a_idx) = line.find("A=") {
                let part = &line[a_idx..];
                if let Some(colon_idx) = part.find(':') {
                    if let Some(space_idx) = part.find(' ') {
                        if colon_idx < space_idx {
                            let pkg = &part[colon_idx + 1..space_idx];

                            if line.contains("visible=true") {
                                visible_pkgs.insert(pkg.to_string());
                            } else if line.contains("visible=false") {
                                invisible_pkgs.insert(pkg.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let exclude = ["com.android.systemui", "com.android.settings", "android", "system"];

    for pkg in invisible_pkgs {
        if visible_pkgs.contains(&pkg) {
            continue;
        }

        let is_excluded = exclude.iter().any(|&ex| pkg.contains(ex));

        if !is_excluded {
            let _ = Command::new("am")
                .args(["force-stop", &pkg])
                .status();

            log_debug(comp::SETTINGS_APPS, &format!("stopped app pkg={}", pkg));
        }
    }

    log_info(comp::SETTINGS_APPS, &format!("cleared background apps"));
}
