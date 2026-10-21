//! Semua path data/runtime AZenith, digenerate saat compile dari
//! `Config.toml` di root repo (lihat `Cargo/build.rs`).
//!
//! Jangan menulis path hardcoded di module lain — tambahkan key-nya di
//! `Config.toml` dan `DIR_KEYS`/`FILE_KEYS` di `build.rs`, lalu pakai
//! konstanta dari sini.
//!
//! Konstanta yang tersedia (semua absolut kecuali `SOC`):
//! `BASE_DIR` `GAMELIST_DIR` `API_DIR` `DAEMON_LOCK` `CODES_CACHE`
//! `STATE_FILE` `BINDER_RESOLVER_APK` `GAMELIST_FILE`
//! `CURRENT_PROFILE` `GAMEINFO` `SOC`
//!
//! `BASE_DIR` `GAMELIST_DIR` dan `API_DIR` dibuat otomatis saat daemon start.
//! `STATE_FILE`, `CURRENT_PROFILE` dan `GAMEINFO` semuanya di `API_DIR` —
//! satu folder buat semua state yang bisa dibaca instance lain.
//! `GAMELIST_DIR` juga menampung flag `gamelistloaded` (`daemon::applist`) —
//! sengaja bukan konstanta, karena flag itu milik `daemon/applist.rs`, bukan
//! konfigurasi build.
//!
//! `SOC` bukan path — nilai `[DEVICE] SOC` dari Config.toml (`none`, `mediatek`,
//! `snapdragon`, `exynos`, `unisoc` atau `tensor`).

include!(concat!(env!("OUT_DIR"), "/paths.rs"));
