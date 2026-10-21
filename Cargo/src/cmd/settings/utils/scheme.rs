//! `persist.azenithconf.schemeconfig` — the display colour scheme.
//!
//! The property holds one of the preset *names* the AZenith app offers:
//! `default`, `vivid`, `warm`, `cool`, `amoled`. It is matched case-insensitively
//! against [`SCHEME_PRESETS`] and spelled out into the four numbers `initialize`
//! sends to SurfaceFlinger.
//!
//! There is no custom scheme: the slider values the app used to store as four raw
//! numbers are not read any more, so any value that is not a known preset name —
//! leftover numbers, `custom`, a typo — resolves to `None` and the display is left
//! alone. Guessing on a display matrix is worse than doing nothing.
//!
//! An unset or empty property resolves to `[1000; 4]`, which `initialize`
//! short-circuits on — no preset, no SurfaceFlinger call, no surprise.

use crate::cmd::settings::utils::getprop;

/// Neutral value: every channel at `1000`.
pub(crate) const SCHEME_DEFAULT: [u32; 4] = [1000, 1000, 1000, 1000];

/// Preset name -> `R G B S`, mirroring `ColorPreset` in the app.
const SCHEME_PRESETS: &[(&str, [u32; 4])] = &[
    ("default", [1000, 1000, 1000, 1000]),
    ("vivid", [1000, 1000, 1000, 1250]),
    ("warm", [1050, 1000, 950, 1100]),
    ("cool", [950, 950, 1050, 1000]),
    ("amoled", [1020, 1020, 1020, 1150]),
];

/// Resolve the property to `Some(R G B S)` when the daemon should apply a
/// scheme, or `None` when it must leave the display alone.
pub(crate) fn schemeconfig() -> Option<[u32; 4]> {
    let raw = getprop("persist.azenithconf.schemeconfig");
    let value = raw.trim();
    if value.is_empty() {
        return Some(SCHEME_DEFAULT);
    }
    preset_by_name(value)
}

/// Look `name` up in [`SCHEME_PRESETS`]. Case-insensitive on purpose: the app
/// writes lower case today, but that is not worth breaking over.
fn preset_by_name(name: &str) -> Option<[u32; 4]> {
    let lowered = name.to_ascii_lowercase();
    SCHEME_PRESETS
        .iter()
        .find(|(key, _)| *key == lowered)
        .map(|(_, preset)| *preset)
}
