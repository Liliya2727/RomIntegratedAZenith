# AZenith Service 

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

---

## 8. Game Package List (`gamelist.txt`)

`gamelist.txt` is auto-loaded on first boot. If no games are detected yet, add packages yourself at any time — **no reboot needed**.

Edit Gamelist `$BASE_DIR/gamelist.txt` (or with custom path) with one package per line:

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

---

## 10. Runtime Output Files

All runtime files are produced by the service while it runs and are placed under `BASE_DIR` (default `/data/vendor/AZenithInt/`). These paths are resolved at compile time by `Cargo/build.rs` from `Config.toml`, so no path is hardcoded in the Rust sources.

Default directory structure:

```
BASE_DIR/                          # /data/vendor/AZenithInt (default)
├── .lock                          # instance lock (DAEMON_LOCK)
├── codes.<sdk>.<incremental>      # binder transaction code cache (CODES_CACHE)
├── binder_resolver.apk            # extracted from the binary at runtime, then deleted
├── gamelist/
│   ├── gamelist.txt               # game package list (GAMELIST_FILE)
│   └── gamelistloaded             # first-boot scan marker (games-v1 <n>)
└── API/                           # state store shared by all instances
    ├── current_profile            # applied profile mode number (0/1/2/3)
    ├── gameinfo                   # "<pkg> <pid> <uid>\nTime: HH:MM:SS\n"
    ├── saved_governor               # "<boot_id>\n<gov>\n" (boot-ID tagged)
    └── device_info.toml           # state store (profile, active app, screen, battery, ...)
```

| File | Produced by | Contents / meaning |
|---|---|---|
| `.lock` | Daemon (`main.rs`) | Exclusive instance lock so only one daemon runs at a time. |
| `disabletweak` (prop) | `persist.sys.azenithIntg.disabletweak` | `1` = block all tweaks (profile still decided, no sysfs/governor writes), `0` = normal tweak apply. Read at runtime by `daemon::profiler`. |
| `codes.<sdk>.<incremental>` | `daemon::codes` | Binder transaction code cache per SDK/build so the resolver APK does not need to run on every boot. |
| `binder_resolver.apk` | Extracted from binary (`include_bytes!`) | Used once to resolve transaction codes, then deleted. |
| `gamelist/gamelist.txt` | `daemon::applist` (first boot) / user | One package name per line. Watched via inotify; changes automatically when edited. |
| `gamelist/gamelistloaded` | `daemon::applist` | Marker that the first-boot game scan is done. Contains `games-v1 <n>`. Delete and reboot to rebuild the list from scratch. |
| `API/current_profile` | Profiler worker | Currently applied profile mode as a number (`0`=none, `1`=performance, `2`=balanced, `3`=eco). |
| `API/gameinfo` | Profiler (`write_gameinfo`) | Foreground package, PID, UID, and timestamp, e.g. `com.example.game 1234 10123\nTime: 14:32:01\n`. |
| `API/saved_governor` | `common::cpugov::save()` | CPU governor saved before switching to performance/eco, tagged with `boot_id` so a value from a previous boot is ignored. |
| `API/device_info.toml` | `daemon::state::write_state()` | Full state store readable by other instances (see example below). |

Example contents of `API/device_info.toml` (auto-generated, do not edit manually):

```toml
# state (auto-generated, do not edit manually)
profile = "balanced"
active_app = "com.example.game"
screen = "on"
battery_saver = "off"
zen_mode = "off"
battery_level = 87
charging = "discharging"
```

This file is written atomically (temp file then rename) and only rewritten when its contents change, so there are no excessive storage writes.

Path rules (`Config.toml`):
* A value starting with `/` = absolute path, used as-is.
* Any other value = relative to its parent directory (e.g. `GAMELIST_DIR` relative to `BASE_DIR`, `GAMELIST_FILE` relative to `GAMELIST_DIR`).
* An empty value `""` = use the parent path.
* `BASE_DIR` must be absolute; otherwise the default is used and `build.rs` prints a `cargo:warning`.
* All paths are compiled into Rust constants in `Cargo/src/common/paths.rs` via `$OUT_DIR/paths.rs`.

---

## 11. The Service (`init.azenith.rc`)

AZenith runs as a vendor service launched by Android `init`. The service definition lives in `init.azenith.rc` (copy it to `/vendor/etc/init/` on the device):

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

### Service properties

| Property | Value | Meaning |
|---|---|---|
| Service name | `AZenith` | Must match the binary path and the `start AZenith` trigger. |
| Binary | `/vendor/bin/hw/vendor.azenith-service` | The compiled release binary from `output/`. |
| SELinux context | `u:r:azenith_service:s0` | Service domain; requires the sepolicy patch from `sepolicyguide.md`. |
| Class | `main` | Starts with the main init class. |
| User / Group | `root` / `root system shell readproc sdcard_r sdcard_rw media_rw` | Root access plus groups needed to read procfs, sysfs, and media storage. |
| Capabilities | `SYS_NICE DAC_OVERRIDE KILL SYS_ADMIN NET_ADMIN SYS_PTRACE` | Needed for CPU tuning, killing background apps, sysfs/procfs writes, network admin, and process inspection. |
| Socket | `zygote stream 660 root system` | Unix socket exposed to zygote-related components. |
| State | `disabled` | Disabled by default; started by the property trigger below. |

### Start trigger

The service starts automatically on boot when both conditions are true:

* `sys.boot_completed=1` — Android has finished booting.
* `persist.sys.azenithIntg.state=1` — master switch enabled in `/vendor/build.prop`.

```sh
# Enable the service in build.prop so it starts on boot:
persist.sys.azenithIntg.state=1
```

The daemon performs a root check at startup and exits with `run as root` if not running as UID 0. It also takes an exclusive lock on `DAEMON_LOCK` so only one instance runs at a time; a second instance exits with `another instance is already running`.

### Service lifecycle

1. `init` starts `AZenith` after boot completes and the state property is `1`.
2. The daemon loads binder transaction codes from its embedded resolver APK, loads/watches `gamelist.txt`, and registers an `IProcessObserver` with the Activity Manager.
3. It runs the boot-phase `initialize` profile, starts its binder thread pools, registers status callbacks (screen, battery saver, zen, battery), and seeds the process table.
4. It then waits for AMS callbacks and applies performance/balanced/eco profiles as the foreground app and system status change.
5. If the Activity Manager dies, the observer callback exits the process so `init` restarts it.

To restart the service manually:

```sh
stop AZenith
start AZenith
```

Or toggle the master property:

```sh
setprop persist.sys.azenithIntg.state 0   # stop on next trigger evaluation
setprop persist.sys.azenithIntg.state 1   # enable
```

Logs are written to logcat under the `AZenith` tag:

```sh
adb logcat -s AZenith
```
