//! Helpers and functions of the `utility` command (the former `binutils` crate).
//!
//! Everything is a thin layer over [`crate::common`]: properties, the forked
//! shell and node writes come from there. What is left is the CPU/governor,
//! thermal, trim and renderer work itself.

pub(crate) mod logging;
pub(crate) use self::logging::*;

use std::thread;
use std::time::Duration;

use glob::glob;

use crate::common::log::comp;
use crate::common::{fs as common_fs, prop, shell};

/// `persist.azenithconf.fstrim` — whether the module wants the trim run.
pub(crate) fn get_fstrim_state() -> String {
    prop::getprop("persist.azenithconf.fstrim")
}

/// Apply the CPU governor to every policy, then log the outcome once.
///
/// The node writes go through [`crate::common::fs::write_val`], which handles
/// the `chmod 644 → write → chmod 444` dance and logs each node once under
/// `[common.fs]`; this function only reports the aggregate result.
pub(crate) fn setsgov(gov: &str) {
    match glob("/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor") {
        Ok(paths) => {
            let mut applied = false;
            for path in paths.flatten() {
                if let Some(p_str) = path.to_str() {
                    if common_fs::write_val(gov, p_str, &common_fs::node_name(p_str), true, comp::UTILITY_CPU) {
                        applied = true;
                    }
                }
            }
            if applied {
                log_info(comp::UTILITY_CPU, &format!("governor applied gov={}", gov));
            } else {
                log_warn(comp::UTILITY_CPU, &format!("governor not applied gov={} reason=no_paths", gov));
            }
        }
        Err(e) => log_error(comp::UTILITY_CPU, &format!("governor glob failed err={}", e)),
    }
}

/// Start or stop the thermal-core companion (`sys.azenith-rianixiathermalcore`).
pub(crate) fn setthermalcore(state: &str) {
    if state == "1" {
        if shell::system(comp::UTILITY_SHELL, "sys.azenith-rianixiathermalcore &") != 0 {
            log_error(comp::UTILITY_THERMAL, "spawn failed");
            return;
        }
        thread::sleep(Duration::from_millis(200));
        match shell::capture(comp::UTILITY_SHELL, "pgrep -f sys.azenith-rianixiathermalcore") {
            Some(pid) if !pid.is_empty() => log_info(comp::UTILITY_THERMAL, &format!("started pid={}", pid)),
            Some(_) => log_warn(comp::UTILITY_THERMAL, "started but pid not found"),
            None => log_error(comp::UTILITY_THERMAL, "pgrep failed"),
        }
    } else if shell::system(comp::UTILITY_SHELL, "pkill -9 -f sys.azenith-rianixiathermalcore") != 0 {
        log_error(comp::UTILITY_THERMAL, "stop failed");
    } else {
        log_info(comp::UTILITY_THERMAL, "stopped");
    }
}

/// Run `fstrim` through the framework, falling back to `sm fstrim`.
pub(crate) fn fstrim() {
    if get_fstrim_state() != "1" {
        return;
    }

    log_debug(comp::UTILITY_FSTRIM, "trying native framework");
    let mut status = shell::system(comp::UTILITY_SHELL, "vdc fstrim dotrim");
    if status != 0 {
        log_debug(comp::UTILITY_FSTRIM, "native failed, retrying via sm");
        status = shell::system(comp::UTILITY_SHELL, "sm fstrim");
    }

    if status == 0 {
        log_info(comp::UTILITY_FSTRIM, "trimmed unused blocks");
    } else {
        log_error(comp::UTILITY_FSTRIM, &format!("all trim commands failed status={}", status));
    }
}

pub(crate) fn enable_dnd() {
    if shell::system(comp::UTILITY_SHELL, "cmd notification set_dnd priority") == 0 {
        log_info(comp::UTILITY_DND, "enabled");
    } else {
        log_error(comp::UTILITY_DND, "enable failed");
    }
}

pub(crate) fn disable_dnd() {
    if shell::system(comp::UTILITY_SHELL, "cmd notification set_dnd off") == 0 {
        log_info(comp::UTILITY_DND, "disabled");
    } else {
        log_error(comp::UTILITY_DND, "disable failed");
    }
}

/// Ask the module's refresh-rate receiver to switch the display to `rate` fps.
pub(crate) fn setrefreshrates(rate: &str) {
    let target_fps = rate.parse::<i32>().unwrap_or(60);

    let status = shell::system(comp::UTILITY_SHELL, &format!(
        "am broadcast -a zx.azenith.SET_FPS -n zx.azenith/.RefreshRateReceiver --ei fps {}",
        target_fps
    ));

    if status == 0 {
        log_info(comp::UTILITY_REFRESH, &format!("applied fps={}", target_fps));
    } else {
        log_warn(comp::UTILITY_REFRESH, "receiver did not report success");
    }
}

/// Restart the whole module: kill the daemons, mark the state stopped, run
/// `service.sh` again.
pub(crate) fn restartservice() {
    let _ = shell::system(comp::UTILITY_SHELL, "pkill -9 -f sys.azenith-rianixiathermalcore");
    let _ = shell::system(comp::UTILITY_SHELL, "pkill -9 -f sys.azenith-service");
    let _ = shell::system(comp::UTILITY_SHELL, "pkill -9 -f sys.azenith-appmonitoring");

    prop::setprop("persist.azenith.state", "stopped");

    if shell::system(comp::UTILITY_SHELL, "sh /data/adb/modules/AZenith/service.sh &") != 0 {
        log_error(comp::DAEMON, "service restart failed");
    } else {
        log_info(comp::DAEMON, "services restarted");
    }
}

/// Apply a HWUI renderer selection (or reset it back to the platform default).
pub(crate) fn setrender(renderer: &str) {
    if renderer == "default" || renderer.is_empty() {
        prop::setprop("debug.hwui.renderer", "");
        prop::setprop("debug.renderengine.backend", "");
        prop::setprop("debug.hwui.render_thread", "");
        prop::setprop("debug.skia.threaded_mode", "");
        prop::resetprop("ro.hwui.use_vulkan", "");

        log_info(comp::UTILITY_RENDERER, "resetting renderers to default");
        return;
    }

    prop::setprop("debug.hwui.renderer", renderer);

    if renderer.contains("threaded") {
        prop::setprop("debug.hwui.render_thread", "true");
        prop::setprop(
            "debug.skia.threaded_mode",
            if renderer.contains("skia") { "true" } else { "false" },
        );
    } else {
        prop::setprop("debug.hwui.render_thread", "false");
        prop::setprop("debug.skia.threaded_mode", "false");
    }

    match renderer {
        "skiavk" | "skiavkthreaded" | "vulkan" => {
            prop::setprop("debug.renderengine.backend", "vulkan");
            prop::resetprop("ro.hwui.use_vulkan", "true");
        }
        "skiagl" | "skiaglthreaded" | "gles" | "opengl" | "openglthreaded" => {
            prop::setprop("debug.renderengine.backend", "gles");
            prop::resetprop("ro.hwui.use_vulkan", "false");
        }
        "software" => {
            prop::setprop("debug.renderengine.backend", "");
            prop::resetprop("ro.hwui.use_vulkan", "false");
        }
        _ => {
            if renderer.contains("vk") || renderer.contains("vulkan") {
                prop::setprop("debug.renderengine.backend", "vulkan");
                prop::resetprop("ro.hwui.use_vulkan", "true");
            } else if renderer.contains("gl") || renderer.contains("gles") {
                prop::setprop("debug.renderengine.backend", "gles");
                prop::resetprop("ro.hwui.use_vulkan", "false");
            } else {
                prop::setprop("debug.renderengine.backend", "");
            }
        }
    }
    log_info(comp::UTILITY_RENDERER, &format!("renderer applied renderer={}", renderer));
}

/// Print `true`/`false` and exit 0/1 — the caller reads the exit status, not
/// the text, but keeping the text makes the command usable by hand.
pub(crate) fn check_mali_path() {
    if has_mali_node() {
        println!("true");
        std::process::exit(0);
    } else {
        println!("false");
        std::process::exit(1);
    }
}

/// `true` when a Mali devfreq node exists, so the caller knows whether the GPU
/// tweaks have anything to act on.
fn has_mali_node() -> bool {
    glob("/sys/class/devfreq/*.mali")
        .map(|mut paths| paths.next().is_some())
        .unwrap_or(false)
}
