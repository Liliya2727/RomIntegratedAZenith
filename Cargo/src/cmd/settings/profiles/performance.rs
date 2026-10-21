//! Profile **performance** (gaming).
//!
//! Follows the taxonomy order defined in [`super::common`]:
//! guard → resolve governor → apply governor → freq → log freq →
//! common tweaks → clear bg apps → chipset → log.

use crate::common::log::comp;
use super::common::{
    apply_common_tweaks, apply_cpu_freq, clear_background_apps_if_enabled, BlockIoTweaks,
    CommonTweaks, CpuFreqMode,
};
use crate::cmd::settings::chipsets::{apply_chipset, ChipsetProfile};
use crate::cmd::settings::utils::*;


const BLOCK_IO: BlockIoTweaks = BlockIoTweaks {
    read_ahead_kb: "32",
    nr_requests: "32",
};

const TWEAKS: CommonTweaks = CommonTweaks {
    vfs_cache_pressure: "80",
    drop_caches: Some("3"),
    power_efficient: "N",
    eas_enabled: "0",
    stune_boost: "30",
    stune_boost_enabled: "1",
    battery_saver: "0",
    battery_saver_bool: "N",
    split_lock_mitigate: "0",
    sched_features: &["NEXT_BUDDY", "NO_TTWU_QUEUE"],
    block_io: Some(BLOCK_IO),
};

pub fn performance_profile() {
    // 1. Guard
    if is_tweak_disabled() {
        return;
    }

    let lite_mode = get_litemode();

    // 2. Governor: save the baseline once, then switch to performance
    apply_performance_governor();

    // 4. CPU frequency
    apply_cpu_freq(CpuFreqMode::MaxAvailable);

    // 5. Log frequency mode
    if !lite_mode {
        log_info(comp::SETTINGS_CPU, &format!("set freq mode=max_available"));
    } else {
        log_info(comp::SETTINGS_CPU, &format!("set freq mode=normal"));
    }

    // 6. Shared tweaks
    apply_common_tweaks(&TWEAKS);

    // 7. Clear background apps
    clear_background_apps_if_enabled();

    // 8. Chipset-specific tweaks (skipped in Lite Mode)
    if !lite_mode {
        apply_chipset(ChipsetProfile::Performance);
    }

    // 9. Done
    log_info(comp::SETTINGS_PROFILE, &format!("performance profile applied"));
}
