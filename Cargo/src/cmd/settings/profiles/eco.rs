//! Profile **eco** (power saving).
//!
//! Follows the taxonomy order defined in [`super::common`]:
//! guard → resolve governor → apply governor → freq → log freq →
//! common tweaks → chipset → log.

use crate::common::log::comp;
use super::common::{apply_common_tweaks, apply_cpu_freq, CommonTweaks, CpuFreqMode};
use crate::cmd::settings::chipsets::{apply_chipset, ChipsetProfile};
use crate::cmd::settings::utils::*;


const TWEAKS: CommonTweaks = CommonTweaks {
    vfs_cache_pressure: "120",
    drop_caches: None,
    power_efficient: "Y",
    eas_enabled: "1",
    stune_boost: "0",
    stune_boost_enabled: "0",
    battery_saver: "1",
    battery_saver_bool: "Y",
    split_lock_mitigate: "1",
    sched_features: &["NO_NEXT_BUDDY", "NO_TTWU_QUEUE"],
    block_io: None,
};

pub fn eco_mode() {
    // 1. Guard
    if is_tweak_disabled() {
        return;
    }

    // 2. Governor: save the baseline once, then switch to powersave
    apply_powersave_governor();

    // 4. CPU frequency (min freq raised to 40% of max inside setfreq)
    apply_cpu_freq(CpuFreqMode::Normal);

    // 5. Log frequency mode
    log_info(comp::SETTINGS_CPU, &format!("set freq mode=low"));

    // 6. Shared tweaks
    apply_common_tweaks(&TWEAKS);

    // 7. Clear background apps: performance profile only

    // 8. Chipset-specific tweaks
    apply_chipset(ChipsetProfile::Powersave);

    // 9. Done
    log_info(comp::SETTINGS_PROFILE, &format!("eco profile applied"));
}
