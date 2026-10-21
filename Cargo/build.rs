// build.rs — konfigurasi build AZenith.
//
// Membaca `../Config.toml` (root repo) lalu men-generate `$OUT_DIR/paths.rs`
// berisi satu `pub(crate) const &str` untuk tiap nilai yang dikonfigurasi:
// semua path runtime dari tabel `[PATHS]`, plus pilihan device dari `[DEVICE]`.
//
// Nama key di Config.toml sengaja dibuat sama persis dengan nama konstanta
// Rust yang di-generate (`BASE_DIR` -> `pub(crate) const BASE_DIR`), jadi tidak
// ada lagi pemetaan dua arah yang bisa meleset.
//
// Semua diselesaikan di sini (build-time), bukan di runtime, supaya:
//   * binary tidak perlu membawa parser TOML (dependency tetap minim), dan
//   * source .rs bebas hardcode konfigurasi — cukup edit Config.toml lalu build ulang.
//
// Aturan resolusi path ada di Config.toml: nilai diawali "/" = absolut, selain itu
// relatif terhadap induknya, dan "" = memakai path induk.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// Key berupa direktori: (nama konstanta = nama key, default, induk).
/// Induk `None` berarti key ini adalah root (`BASE_DIR`) dan nilainya harus absolut.
const DIR_KEYS: &[(&str, &str, Option<&str>)] = &[
    ("BASE_DIR", "/data/adb/.config/AntaresRS", None),
    ("GAMELIST_DIR", "gamelist", Some("BASE_DIR")),
    ("API_DIR", "API", Some("BASE_DIR")),
];

/// Key berupa file: (nama konstanta = nama key, default, induk).
/// Default-nya relatif terhadap induknya, jadi mengubah `GAMELIST_DIR` juga
/// memindahkan `GAMELIST_FILE` secara otomatis.
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

/// `[DEVICE] SOC` — keluarga SoC yang tweak-nya dipakai. `none` = tanpa tweak khusus,
/// sama seperti perilaku lama saat prop `soctype` kosong.
const SOC_DEFAULT: &str = "none";

/// Nilai yang diterima untuk `[DEVICE] SOC` -> nama kanonik yang di-generate.
/// Angka lama (dari prop `persist.azenith.soctype`) masih diterima untuk migrasi,
/// tapi build.rs bakal kasih peringatan.
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

    // src/daemon/codes.rs nge-embed file ini lewat include_bytes!(). File-nya tidak
    // ikut di repo (CI menyalinnya dari hasil build Resolver/ sebelum cargo jalan),
    // jadi dicek di sini dulu — daripada error-nya cuma nunjuk baris .rs yang bikin
    // bingung.
    let apk = manifest.join("binder_resolver.apk");
    println!("cargo:rerun-if-changed={}", apk.display());
    if !apk.is_file() {
        panic!(
            "Cargo/binder_resolver.apk tidak ada.\n\
             Build Binder Resolver dulu, lalu salin APK-nya:\n  \
             cd Resolver && ./gradlew assembleRelease\n  \
             cp app/build/outputs/apk/release/app-release-unsigned.apk ../Cargo/binder_resolver.apk\n\
             (CI melakukannya otomatis di step \"Stage Binder Resolver APK\")"
        );
    }

    let config = manifest.join("..").join("Config.toml");
    println!("cargo:rerun-if-changed={}", config.display());

    let raw = match fs::read_to_string(&config) {
        Ok(text) => text,
        Err(e) => {
            println!("cargo:warning=Config.toml tidak bisa dibaca ({e}), memakai semua path default");
            String::new()
        }
    };
    let values = parse_section(&raw, "PATHS");

    // Pass 1: direktori (BASE_DIR lebih dulu karena semua key lain turun darinya).
    let mut resolved: BTreeMap<&'static str, String> = BTreeMap::new();
    for (konst, default, parent) in DIR_KEYS {
        let parent_path = parent.map(|p| resolved[p].as_str()).unwrap_or("");
        let value = values.get(*konst).map(String::as_str).unwrap_or(*default);
        resolved.insert(*konst, solve(*konst, parent_path, value, *default, *parent));
    }
    // BASE_DIR adalah root; semua path lain harus berada di bawahnya secara absolut.
    if !resolved["BASE_DIR"].starts_with('/') {
        println!(
            "cargo:warning=BASE_DIR bukan path absolut ({}), memakai default",
            resolved["BASE_DIR"]
        );
        resolved.insert("BASE_DIR", DIR_KEYS[0].1.to_string());
    }

    // Pass 2: file, relatif terhadap direktori induknya yang sudah final.
    let mut out = String::from(
        "// @generated oleh build.rs dari Config.toml — JANGAN EDIT.\n\
         // Ubah Config.toml di root repo lalu build ulang.\n\n",
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

    // Pass 3: konfigurasi device dari tabel [DEVICE].
    out.push_str(&format!(
        "pub(crate) const SOC: &str = \"{}\";\n",
        escape(&resolve_soc(&parse_section(&raw, "DEVICE")))
    ));

    let dest = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("paths.rs");
    fs::write(&dest, out).unwrap_or_else(|e| panic!("gagal menulis {}: {e}", dest.display()));
}

/// Gabungkan `value` dengan `parent` lalu validasi hasilnya absolut.
fn solve(key: &str, parent: &str, value: &str, default: &str, has_parent: Option<&str>) -> String {
    let joined = join(parent, value);
    if joined.starts_with('/') {
        return joined;
    }
    // Hasilnya tidak absolut — biasanya BASE_DIR yang salah/kosong. Kembali ke default.
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
/// `join("/a", "")` = "/a". Slash ganda dan slash di akhir dibersihkan.
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

/// Ambil hanya key dari satu tabel di Config.toml: `key = "value"` atau `key = value`.
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

/// Normalisasi `[DEVICE] SOC` ke nama kanonik yang di-generate. Angka lama
/// (0-5, bekas prop `soctype`) masih diterima supaya migrasi tidak diam-diam putus.
fn resolve_soc(device: &BTreeMap<String, String>) -> String {
    let Some(value) = device.get("SOC").map(|v| v.trim().to_ascii_lowercase()) else {
        return SOC_DEFAULT.to_string();
    };
    if value.is_empty() {
        return SOC_DEFAULT.to_string();
    }
    let Some((alias, canonical)) = SOC_ALIASES.iter().find(|(alias, _)| *alias == value) else {
        println!(
            "cargo:warning=[DEVICE] SOC = \"{value}\" tidak dikenal, memakai \"{SOC_DEFAULT}\" \
             (nilai valid: none, mediatek, snapdragon, exynos, unisoc, tensor)"
        );
        return SOC_DEFAULT.to_string();
    };
    if alias.chars().all(|c| c.is_ascii_digit()) {
        println!(
            "cargo:warning=[DEVICE] SOC = \"{alias}\" sudah legacy (bekas prop soctype), pakai \"{canonical}\""
        );
    }
    canonical.to_string()
}

/// Buang komentar `#` yang berada di luar string.
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

/// Lepas pasangan tanda kutip dan proses escape dasar.
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

/// Escape supaya aman dipakai di dalam string literal Rust.
fn escape(path: &str) -> String {
    path.replace('\\', "\\\\").replace('"', "\\\"")
}
