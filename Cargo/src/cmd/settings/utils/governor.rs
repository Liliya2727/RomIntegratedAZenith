//! CPU governor detection, saving/restoring and installation, plus the HWUI
//! renderer.
//!
//! The I/O scheduler and the Mali GPU governor used to be applied from here as
//! well; both are gone now — the block scheduler is left at whatever the kernel
//! picked and the GPU keeps its own governor.
//!
//! Governor bookkeeping: `initialize` applies the baseline once, `performance`
//! and `eco` save it before switching, and `balanced` puts it back. See
//! [`crate::common::cpugov`].

use crate::common::cpugov;
use crate::common::log::comp;
use crate::cmd::settings::utils::log_info;
use crate::cmd::settings::utils::{getprop, resetprop, setprop};
use crate::cmd::settings::utils::common_chmod as chmod;
use std::fs;

/// Governor currently set on cpu0 — the kernel baseline, read fresh from sysfs.
pub(crate) fn current() -> String {
    fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(crate) fn setgov(gov: &str) {
    if let Ok(paths) = glob::glob("/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor") {
        for path in paths.flatten() {
            let p_str = path.to_str().unwrap();
            chmod(p_str, 0o644);
            let _ = fs::write(p_str, gov);
            chmod(p_str, 0o444);
        }
    }

    // Lock additional policy paths
    if let Ok(paths) = glob::glob("/sys/devices/system/cpu/cpufreq/policy*/scaling_governor") {
        for path in paths.flatten() {
            chmod(path.to_str().unwrap(), 0o444);
        }
    }
}

/// Apply `gov` to every policy and log it once.
fn apply(gov: &str, why: &str) {
    setgov(gov);
    log_info(comp::SETTINGS_CPU, &format!("{why} gov={gov}"));
}

/// `balanced` — the kernel baseline.
///
/// `custom_default_cpu_gov` still wins; otherwise the governor saved before the
/// last `performance`/`eco` is put back. On the very first apply after boot
/// nothing is saved yet, so the kernel baseline is adopted and left alone.
pub(crate) fn apply_balanced_governor() {
    let custom = getprop("persist.sys.azenithIntg.custom_default_cpu_gov");
    if !custom.is_empty() {
        apply(&custom, "using custom governor");
        return;
    }

    let saved = cpugov::saved();
    if saved.is_empty() {
        log_info(comp::SETTINGS_CPU, &format!("no governor saved yet, leaving kernel baseline gov={}", current()));
        return;
    }
    apply(&saved, "restoring default governor");
}

/// `performance` — save the baseline first, then switch.
pub(crate) fn apply_performance_governor() {
    apply_profile_governor("persist.sys.azenithIntg.custom_performance_cpu_gov", "performance");
}

/// `eco` — same contract as [`apply_performance_governor`].
pub(crate) fn apply_powersave_governor() {
    apply_profile_governor("persist.sys.azenithIntg.custom_powersave_cpu_gov", "powersave");
}

/// A `performance`/`eco` apply: an explicit `custom_*` property wins untouched.
/// Without one, the governor on cpu0 *right now* is saved, then the mode
/// governor is applied — and that save overwrites whatever the previous mode
/// switch left behind (see [`crate::common::cpugov::save`]).
fn apply_profile_governor(custom_key: &str, mode_gov: &str) {
    let custom = getprop(custom_key);
    if !custom.is_empty() {
        apply(&custom, "using custom governor");
        return;
    }

    let baseline = current();
    cpugov::save(&baseline);
    if !baseline.is_empty() {
        log_info(comp::SETTINGS_CPU, &format!("saved default governor gov={baseline}"));
    }
    apply(mode_gov, "applying mode governor");
}

pub(crate) fn setrender(renderer: &str) {
    if renderer == "default" || renderer.is_empty() {
        setprop("debug.hwui.renderer", "");
        setprop("debug.renderengine.backend", "");
        setprop("debug.hwui.render_thread", "");
        setprop("debug.skia.threaded_mode", "");
        resetprop("ro.hwui.use_vulkan", ""); 
        
        log_info(comp::SETTINGS_RENDERER, &format!("reset to default"));
        return;
    }

    setprop("debug.hwui.renderer", renderer);

    if renderer.contains("threaded") {
        setprop("debug.hwui.render_thread", "true");
        if renderer.contains("skia") {
            setprop("debug.skia.threaded_mode", "true");
        } else {
            setprop("debug.skia.threaded_mode", "false");
        }
    } else {
        setprop("debug.hwui.render_thread", "false");
        setprop("debug.skia.threaded_mode", "false");
    }

    match renderer {
        "skiavk" | "skiavkthreaded" | "vulkan" => {
            setprop("debug.renderengine.backend", "vulkan");
            resetprop("ro.hwui.use_vulkan", "true"); 
        }
        "skiagl" | "skiaglthreaded" | "gles" | "opengl" | "openglthreaded" => {
            setprop("debug.renderengine.backend", "gles");
            resetprop("ro.hwui.use_vulkan", "false");
        }
        "software" => {
            setprop("debug.renderengine.backend", "");
            resetprop("ro.hwui.use_vulkan", "false");
        }
        _ => {
            if renderer.contains("vk") || renderer.contains("vulkan") {
                setprop("debug.renderengine.backend", "vulkan");
                resetprop("ro.hwui.use_vulkan", "true");
            } else if renderer.contains("gl") || renderer.contains("gles") {
                setprop("debug.renderengine.backend", "gles");
                resetprop("ro.hwui.use_vulkan", "false");
            } else {
                setprop("debug.renderengine.backend", "");
            }
        }
    }
    log_info(comp::SETTINGS_RENDERER, &format!("renderer applied renderer={}", renderer));
}

pub(crate) fn init_renderer() {
    let renderer = getprop("persist.sys.azenithConfIntg.renderer");
    
    if renderer.is_empty() || renderer.eq_ignore_ascii_case("default") {
        log_info(comp::SETTINGS_RENDERER, &format!("default renderer, skipping setup"));
        return;
    }
    
    log_info(comp::SETTINGS_RENDERER, &format!("applying renderer renderer={}", renderer));
    setrender(&renderer);
}
