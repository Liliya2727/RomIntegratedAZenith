// ---------------- installed game packages, from the package manager ----------------
//
// The first-boot gamelist scan must not contain every installed package: a
// launcher, a browser or a settings app is not a game, and applying a game
// profile to them only wastes work. The package manager already knows which
// applications are games (`ApplicationInfo.category == CATEGORY_GAME`), but
// that field lives inside a `ParceledListSlice<ApplicationInfo>`, which this
// process cannot decode with the symbols libbinder_ndk exports.
//
// The embedded resolver APK can decode it in Java. It answers the special
// `GAMES_QUERY` question with a marker-delimited list of package names, so the
// rest of the daemon stays free of Parcel/ApplicationInfo layout knowledge.

use std::collections::BTreeSet;

use crate::common::log::{comp, debug, warn};

/// Pseudo-question understood by the resolver APK (Resolver/app/.../MainKt.kt).
const GAMES_QUERY: &str = "__AZENITH_GAMES__";
const GAMES_BEGIN: &str = "AZENITH_GAMES_BEGIN";
const GAMES_END: &str = "AZENITH_GAMES_END";

/// Package names the scan must never put in the gamelist: the daemon and its
/// companion UI are not games, and a profile applied to them is wasted work.
const NEVER_GAME: [&str; 1] = ["zx.azenith"];

/// Every installed package categorised as a game, or `None` if the resolver
/// could not answer. Callers retry later rather than treating `None` as
/// "no games".
pub(crate) fn installed_games() -> Option<BTreeSet<String>> {
    let output = crate::daemon::codes::run_query(GAMES_QUERY)?;

    let mut in_list = false;
    let mut complete = false;
    let mut games = BTreeSet::new();

    for line in output.lines().map(str::trim) {
        if line.starts_with(GAMES_BEGIN) {
            in_list = true;
        } else if line == GAMES_END {
            complete = true;
            break;
        } else if in_list && looks_like_package(line) {
            games.insert(line.to_string());
        }
    }

    if !complete {
        // An older prebuilt resolver only knows transaction codes. Do not mark
        // the scan as done: the next boot (with an updated APK) can rebuild it.
        warn!(comp::APPLIST, "game scan output incomplete (resolver APK too old?)");
        return None;
    }

    debug!(comp::APPLIST, "game scan found {} installed games", games.len());
    Some(games)
}

/// A package name is a dotted identifier with an optional `:process` suffix.
/// This keeps junk out of the list when a reply was decoded from the wrong
/// layout, and drops the daemon's own companion app.
pub(crate) fn looks_like_package(name: &str) -> bool {
    let name = name.split(':').next().unwrap_or("");
    if !name.contains('.') || name.is_empty() || name.len() > 255 {
        return false;
    }
    if !name.split('.').all(|seg| {
        !seg.is_empty()
            && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && seg.starts_with(|c: char| c.is_ascii_alphabetic())
    }) {
        return false;
    }
    !NEVER_GAME.iter().any(|skip| name == *skip || name.starts_with(&format!("{skip}.")))
}
