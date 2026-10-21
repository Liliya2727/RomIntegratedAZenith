// build.rs — AZenith build configuration.
//
// Reads `../Config.toml` (repo root) and generates `$OUT_DIR/paths.rs` holding
// one `pub(crate) const &str` for every configured value: all runtime paths from
// the `[PATHS]` table, plus the device choice from `[DEVICE]`.
//
// Every key in Config.toml is named exactly like the Rust constant it generates
// (`BASE_DIR` -> `pub(crate) const BASE_DIR`), so there is no two-way mapping
// that can drift.
//
// Everything is resolved here, at build time rather than at runtime, so that:
//   * the binary does not have to carry a TOML parser (dependencies stay minimal), and
//   * the .rs sources stay free of hardcoded config — edit Config.toml and rebuild.
//
// The path resolution rules live in Config.toml: a value starting with "/" is
// absolute, anything else is relative to its parent, and "" means "use the
// parent path".

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// Directory keys: (constant name = key name, default, parent).
/// A `None` parent means this key is the root (`BASE_DIR`) and its value must be absolute.
const DIR_KEYS: &[(&str, &str, Option<&str>)] = &[
    ("BASE_DIR", "/data/adb/.config/AntaresRS", None),
    ("GAMELIST_DIR", "gamelist", Some("BASE_DIR")),
    ("API_DIR", "API", Some("BASE_DIR")),
];

/// File keys: (constant name = key name, default, parent).
/// Defaults are relative to their parent, so changing `GAMELIST_DIR` also moves
/// `GAMELIST_FILE` along with it.
const FILE_KEYS: &[(&str, &str, &str)] = &[
    ("DAEMON_LOCK", "daemon.lock", "BASE_DIR"),
    ("CODES_CACHE", "codes", "BASE_DIR"),
    // state store — lives in API_DIR with CURRENT_PROFILE/GAMEINFO, since it
    // carries the same information (active app, applied profile) they do.
    ("STATE_FILE", "state.toml", "API_DIR"),
    ("BINDER_RESOLVER_APK", "binder_resolver.apk", "BASE_DIR"),
    ("GAMELIST_FILE", "gamelist.txt", "GAMELIST_DIR"),
    ("CURRENT_PROFILE", "current_profile", "API_DIR"),
    ("GAMEINFO", "gameinfo", "API_DIR"),
];

/// `[DEVICE] SOC` — the SoC family whose tweaks get applied. `none` = no chipset
/// specific tweaks, same as the old behaviour when the `soctype` prop was empty.
const SOC_DEFAULT: &str = "none";

/// Values accepted for `[DEVICE] SOC` -> canonical name that gets generated.
/// The old numeric values (from the `persist.azenith.soctype` prop) are still
/// accepted so migrations keep working, but build.rs prints a warning for them.
const SOC_ALIASES: &[(&str, &str)] = &[
    ("none", "none"),
    ("mediatek", "mediatek"),
    ("snapdragon", "snapdragon"),
    ("exynos", "exynos"),
    ("unisoc", "unisoc"),
    ("tensor", "tensor"),
    ("0", "none"),
    ("1", "mediatek"),
    ("2", "snapdragon"),
    ("3", "exynos"),
    ("4", "unisoc"),
    ("5", "tensor"),
];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));

    // src/daemon/codes.rs embeds this file with include_bytes!(). It is not part
    // of the repo (CI copies it over from the Resolver build before cargo runs),
    // so check for it here first — otherwise all you get is an error pointing at
    // a .rs line with no explanation.
    let apk = manifest.join("binder_resolver.apk");
    println!("cargo:rerun-if-changed={}", apk.display());
    if !apk.is_file() {
        panic!(
            "Cargo/binder_resolver.apk is missing.\n\
             Build the Binder Resolver first, then copy the APK over:\n  \
             cd Resolver && ./gradlew assembleRelease\n  \
             cp app/build/outputs/apk/release/app-release-unsigned.apk ../Cargo/binder_resolver.apk\n\
             (CI does this automatically in the \"Stage Binder Resolver APK\" step)"
        );
    }

    let config = manifest.join("..").join("Config.toml");
    println!("cargo:rerun-if-changed={}", config.display());

    let raw = match fs::read_to_string(&config) {
        Ok(text) => text,
        Err(e) => {
            println!("cargo:warning=cannot read Config.toml ({e}), falling back to every default path");
            String::new()
        }
    };
    let values = parse_section(&raw, "PATHS");

    // Pass 1: directories (BASE_DIR first, since every other key derives from it).
    let mut resolved: BTreeMap<&'static str, String> = BTreeMap::new();
    for (konst, default, parent) in DIR_KEYS {
        let parent_path = parent.map(|p| resolved[p].as_str()).unwrap_or("");
        let value = values.get(*konst).map(String::as_str).unwrap_or(*default);
        resolved.insert(*konst, solve(*konst, parent_path, value, *default, *parent));
    }
    // BASE_DIR is the root; every other path has to live under it and be absolute.
    if !resolved["BASE_DIR"].starts_with('/') {
        println!(
            "cargo:warning=BASE_DIR is not an absolute path ({}), using the default",
            resolved["BASE_DIR"]
        );
        resolved.insert("BASE_DIR", DIR_KEYS[0].1.to_string());
    }

    // Pass 2: files, relative to their parent directory which is already final.
    let mut out = String::from(
        "// @generated by build.rs from Config.toml — DO NOT EDIT.\n\
         // Edit Config.toml in the repo root, then rebuild.\n\n",
    );
    for (konst, path) in &resolved {
        out.push_str(&format!("pub(crate) const {konst}: &str = \"{}\";\n", escape(path)));
    }
    for (konst, default, parent) in FILE_KEYS {
        let parent_path = resolved[*parent].as_str();
        let value = values.get(*konst).map(String::as_str).unwrap_or(*default);
        let path = solve(*konst, parent_path, value, *default, Some(*parent));
        out.push_str(&format!("pub(crate) const {konst}: &str = \"{}\";\n", escape(&path)));
    }

    // Pass 3: device configuration from the [DEVICE] table.
    out.push_str(&format!(
        "pub(crate) const SOC: &str = \"{}\";\n",
        escape(&resolve_soc(&parse_section(&raw, "DEVICE")))
    ));

    let dest = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("paths.rs");
    fs::write(&dest, out).unwrap_or_else(|e| panic!("failed to write {}: {e}", dest.display()));
}

/// Join `value` with `parent`, then validate that the result is absolute.
fn solve(key: &str, parent: &str, value: &str, default: &str, has_parent: Option<&str>) -> String {
    let joined = join(parent, value);
    if joined.starts_with('/') {
        return joined;
    }
    // Not absolute — usually a bad or empty BASE_DIR. Fall back to the default.
    let resolved_default = match has_parent {
        Some(_) => join(parent, default),
        None => default.to_string(),
    };
    println!(
        "cargo:warning=path '{key}' = \"{value}\" is not absolute, using the default \"{resolved_default}\""
    );
    resolved_default
}

/// `join("", "x")` = "x"; `join("/a", "/b")` = "/b"; `join("/a", "b")` = "/a/b";
/// `join("/a", "")` = "/a". Duplicate and trailing slashes are cleaned up.
fn join(parent: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return parent.trim().trim_end_matches('/').to_string();
    }
    if value.starts_with('/') {
        return value.trim_end_matches('/').to_string();
    }
    let parent = parent.trim().trim_end_matches('/');
    let value = value.trim_end_matches('/');
    if parent.is_empty() {
        value.to_string()
    } else {
        format!("{parent}/{value}")
    }
}

/// Pull the keys out of a single table in Config.toml: `key = "value"` or `key = value`.
fn parse_section(text: &str, wanted: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut in_section = false;

    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(section) = line.strip_prefix('[') {
            in_section = section.trim().trim_end_matches(']').trim() == wanted;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        out.insert(key.to_string(), unquote(value.trim()));
    }
    out
}

/// Normalize `[DEVICE] SOC` to the canonical name that gets generated. The old
/// numeric values (0-5, from the former `soctype` prop) are still accepted so a
/// migration does not silently break.
fn resolve_soc(device: &BTreeMap<String, String>) -> String {
    let Some(value) = device.get("SOC").map(|v| v.trim().to_ascii_lowercase()) else {
        return SOC_DEFAULT.to_string();
    };
    if value.is_empty() {
        return SOC_DEFAULT.to_string();
    }
    let Some((alias, canonical)) = SOC_ALIASES.iter().find(|(alias, _)| *alias == value) else {
        println!(
            "cargo:warning=[DEVICE] SOC = \"{value}\" is unknown, using \"{SOC_DEFAULT}\" \
             (valid values: none, mediatek, snapdragon, exynos, unisoc, tensor)"
        );
        return SOC_DEFAULT.to_string();
    };
    if alias.chars().all(|c| c.is_ascii_digit()) {
        println!(
            "cargo:warning=[DEVICE] SOC = \"{alias}\" is legacy (from the old soctype prop), use \"{canonical}\""
        );
    }
    canonical.to_string()
}

/// Drop `#` comments that sit outside a string.
fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match c {
            '\\' if in_string => escaped = true,
            '"' if !escaped => in_string = !in_string,
            '#' if !in_string => return &line[..i],
            _ => escaped = false,
        }
    }
    line
}

/// Strip the surrounding quotes and process basic escapes.
fn unquote(value: &str) -> String {
    let value = value.trim();
    let inner = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value);

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Escape so the value is safe to use inside a Rust string literal.
fn escape(path: &str) -> String {
    path.replace('\\', "\\\\").replace('"', "\\\"")
}
