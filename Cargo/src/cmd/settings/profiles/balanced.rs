//! Profile **balanced** (daily use).
//!
//! Follows the taxonomy order defined in [`super::common`]:
//! guard → resolve governor → apply governor → freq → log freq →
//! common tweaks → chipset → log.

use crate::common::log::comp;
use super::common::{
    apply_common_tweaks, apply_cpu_freq, BlockIoTweaks, CommonTweaks, CpuFreqMode,
};
use crate::cmd::settings::chipsets::{apply_chipset, ChipsetProfile};
use crate::cmd::settings::utils::*;


const BLOCK_IO: BlockIoTweaks = BlockIoTweaks {
    read_ahead_kb: "128",
    nr_requests: "64",
};

const TWEAKS: CommonTweaks = CommonTweaks {
    vfs_cache_pressure: "120",
    drop_caches: None,
    power_efficient: "Y",
    eas_enabled: "1",
    stune_boost: "0",
    stune_boost_enabled: "0",
    battery_saver: "0",
    battery_saver_bool: "N",
    split_lock_mitigate: "1",
    sched_features: &["NEXT_BUDDY", "TTWU_QUEUE"],
    block_io: Some(BLOCK_IO),
};

pub fn balanced_profile() {
    // 1. Guard
    if is_tweak_disabled() {
        return;
    }

    // 2. Governor: back to the saved baseline (or custom_default_cpu_gov)
    apply_balanced_governor();

    // 4. CPU frequency
    apply_cpu_freq(CpuFreqMode::Normal);

    // 5. Log frequency mode
    if getprop("persist.azenithconf.freqoffset") == "Disabled" {
        log_info(comp::SETTINGS_CPU, &format!("set freq mode=normal"));
    } else {
        log_info(comp::SETTINGS_CPU, &format!("set freq mode=normal_selected"));
    }

    // 6. Shared tweaks
    apply_common_tweaks(&TWEAKS);

    // 7. Clear background apps: performance profile only

    // 8. Chipset-specific tweaks
    apply_chipset(ChipsetProfile::Balanced);

    // 9. Done
    log_info(comp::SETTINGS_PROFILE, &format!("balanced profile applied"));
}
