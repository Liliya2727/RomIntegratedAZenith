# AZenith Service — Full Docs

Complete technical reference for the AZenith daemon (`vendor.azenith-service`) and how to integrate it into a custom ROM vendor image.

---

## 1. Overview

AZenith is a system-level performance daemon for Android custom ROMs. It runs as a vendor service (`/vendor/bin/hw/vendor.azenith-service`) launched by `init` via `init.azenith.rc`, applies CPU/GI/tweak profiles to game packages listed in `gamelist.txt`, and exposes runtime toggles through system properties under `persist.sys.azenithIntg.*` and `persist.sys.azenithConfIntg.*`.

The daemon is self-contained — no separate `AZenith_Profiler` or `AZenith_Utility` binaries ship anymore. Everything lives in the single `azenith-service` binary.

---

## 2. Build

There is no CI pipeline. Every Rom Maintainer builds locally:

```sh
git clone https://github.com/Liliya2727/RomIntegratedAZenith.git
cd RomIntegratedAZenith
```

### Configure `Config.toml` (before building)

```toml
[DEVICE]
SOC = "mediatek"   # mediatek | snapdragon | exynos | unisoc | tensor | none
```

Optionally relocate runtime paths under `[PATHS]` (see Build Configuration below).

### Compile

```sh
./build_binary.sh             # release build
./build_binary.sh --debug     # debug build
./build_binary.sh --clean     # clean target first
./build_binary.sh --output /path/to/staging
```

`build_binary.sh` builds the Binder Resolver APK (embedded via `include_bytes!`), compiles `azenith-service` for `aarch64-linux-android`, and stages it at `output/vendor.azenith-service`.

---

## 3. Build Configuration (`Config.toml`)

Every runtime path AZenith uses is resolved at compile time by `Cargo/build.rs` into `$OUT_DIR/paths.rs` — nothing is hardcoded in the Rust sources.

Edit [`Config.toml`](Config.toml) **before building**. Set at minimum the chipset:

```toml
[DEVICE]
SOC = "mediatek"
```

Relocate runtime paths under `[PATHS]` if your ROM layout differs:

```toml
[PATHS]
BASE_DIR      = "/data/vendor/AZenithInt"
GAMELIST_DIR  = "gamelist.txt"
# or
GAMELIST_DIR  = "/sdcard/gamelist.txt"
```

| Key | Meaning |
|-----|---------|
| `BASE_DIR` | Root of all AZenith runtime data |
| `GAMELIST_DIR` | Game package list directory |
| `[DEVICE] SOC` | SoC family baked into the binary |

Rules:
- Values starting with `/` are used as-is.
- Other values are relative to their parent directory.
- An empty string means "use the parent path".
- Delete any key you don't need — defaults apply automatically.
- An invalid (non-absolute) path falls back to the default and prints a `cargo:warning`.

One binary carries exactly one chipset; `none` means "only the generic tweaks". Unknown values fall back to `none` with a build warning. Old numeric values (`1`–`5`) are still accepted and normalized to the names above.

---

## 4. Vendor Integration

### How to Add AZenith into my Vendor?

1. Copy `output/vendor.azenith-service` to `/vendor/bin/hw/vendor.azenith-service` (must match the service name in `init.azenith.rc`).
2. Copy `init.azenith.rc` to `/vendor/etc/init/`.
3. No separate helper binaries ship anymore — the daemon is self-contained.
4. Patch the vendor sepolicy — see [sepolicyguide.md](sepolicyguide.md).
5. In `/vendor/build.prop`, add:
   ```
   persist.sys.azenithIntg.state=1   # 1 = enabled, 0 = disabled
   ```
6. Enable other tweaks via the [properties list](listproperties.md).
7. Repack the vendor image.

### `init.azenith.rc`

The service starts automatically on boot when `persist.sys.azenithIntg.state=1`:

```rc
service AZenith /vendor/bin/hw/vendor.azenith-service
    seclabel u:r:azenith_service:s0
    class main
    user root
    group root system shell readproc sdcard_r sdcard_rw media_rw
    capabilities SYS_NICE DAC_OVERRIDE KILL SYS_ADMIN NET_ADMIN SYS_PTRACE
    disabled
    socket zygote stream 660 root system

on property:sys.boot_completed=1 && property:persist.sys.azenithIntg.state=1
    start AZenith
```

---

## 5. SELinux Policy

Patch the vendor sepolicy so `init` can launch the service and the daemon can touch system properties. See [`sepolicyguide.md`](sepolicyguide.md) for the full CIL definitions and file contexts.

Key file context:

```
/vendor/bin/hw/vendor.azenith-service    u:object_r:azenith_service_exec:s0
```

The service needs access to `system_prop` (platform-owned `persist.sys.` prefix):

```cil
(allow azenith_service system_prop (file (getattr map open read)))
(allow azenith_service system_prop (property_service (set)))
```

---

## 6. System Properties

| Property | Values | Meaning |
|----------|--------|---------|
| `persist.sys.azenithIntg.state` | `1` / `0` | Enable / disable the daemon |
| `persist.sys.azenithIntg.debugmode` | `true` / `false` | Verbose DEBUG logcat output |
| `persist.sys.azenithConfIntg.*` | — | Individual tweak toggles (see [properties list](listproperties.md)) |

`persist.sys.` is platform-owned and maps to `system_prop`; do not add AZenith prefixes to `vendor_property_context`.

Verbose output is opt-in:

```sh
setprop persist.sys.azenithIntg.debugmode true   # enable DEBUG
setprop persist.sys.azenithIntg.debugmode false  # back to info
```

Restart the service after toggling `debugmode`.

---

## 7. Logging

AZenith writes to logcat under the `AZenith` tag:

```sh
adb logcat -s AZenith
```

Each line is prefixed with the component, e.g. `[daemon.profiler]` or `[cmd.settings.cpu]`.

For a one-off companion command run, override the level with `LIBBINDING_LOG`:

```sh
LIBBINDING_LOG=debug ./some-companion-command   # debug / info / warn / error
```

---

## 8. Game Package List (`gamelist.txt`)

`gamelist.txt` is auto-loaded on first boot. If no games are detected yet, add packages yourself at any time — **no reboot needed**.

Create `/sdcard/gamelist.txt` with one package per line:

```
com.mobile.legends
adventure.rpg.anime.game.vng.ys6
age.of.civilizations2.jakowski.lukasz
air.com.ace2three.mobile.cash
```

The daemon generates an initial list containing only the installed apps Android categorises as games; you can append your own packages in new lines.

---

## 9. File Layout

Runtime files live under `BASE_DIR` (default `/data/vendor/AZenithInt/`):

```
BASE_DIR/
  gamelist/          # package lists
  API/
    current_profile
    gameinfo
    device_info.toml
  ... binder resolver cache, instance lock
```

Paths declared relative to an overridden directory keep following it; everything is resolved at compile time by `build.rs`.
