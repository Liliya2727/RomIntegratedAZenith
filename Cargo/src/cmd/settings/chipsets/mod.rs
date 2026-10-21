//! Chipset-specific tweaks.
//!
//! The chipset is baked in at build time through `[DEVICE] SOC` in `Config.toml`
//! (exposed here as [`crate::common::paths::SOC`]), so one binary build carries
//! exactly one SoC set instead of probing a property at runtime.
//!
//! Each chipset has three variants sharing the *same step order* — only the
//! written values differ, not which node gets touched first.

pub mod exynos;
pub mod mediatek;
pub mod snapdragon;
pub mod tensor;
pub mod unisoc;

use crate::common::paths::SOC;

/// Profile variant applied to the chipset.
#[derive(Clone, Copy, Debug)]
pub enum ChipsetProfile {
    Balanced,
    Performance,
    Powersave,
}

/// `(balanced, performance, powersave)` tuple for a single SoC.
type ChipsetFns = (fn(), fn(), fn());

/// Dispatch chipset tweaks for the SoC this binary was built for.
pub fn apply_chipset(profile: ChipsetProfile) {
    let (balanced, performance, powersave): ChipsetFns = match SOC {
        "mediatek" => (
            mediatek::mediatek_balance,
            mediatek::mediatek_performance,
            mediatek::mediatek_powersave,
        ),
        "snapdragon" => (
            snapdragon::snapdragon_balance,
            snapdragon::snapdragon_performance,
            snapdragon::snapdragon_powersave,
        ),
        "exynos" => (
            exynos::exynos_balance,
            exynos::exynos_performance,
            exynos::exynos_powersave,
        ),
        "unisoc" => (
            unisoc::unisoc_balance,
            unisoc::unisoc_performance,
            unisoc::unisoc_powersave,
        ),
        "tensor" => (
            tensor::tensor_balance,
            tensor::tensor_performance,
            tensor::tensor_powersave,
        ),
        // "none" (atau nilai yang gagal divalidasi build.rs) -> tanpa tweak khusus.
        _ => return,
    };

    match profile {
        ChipsetProfile::Balanced => balanced(),
        ChipsetProfile::Performance => performance(),
        ChipsetProfile::Powersave => powersave(),
    }
}
