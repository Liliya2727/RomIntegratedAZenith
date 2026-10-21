# AZenith Properties List

> **Renamed:** the old `persist.azenith.*` / `persist.azenithconf.*` properties are now
> `persist.sys.azenithIntg.*` and `persist.sys.azenithConfIntg.*`. If you are updating from an
> older build, migrate your `/vendor/build.prop` values and your property contexts, or nothing
> will be read.
>
> Don't forget the [sepolicy](sepolicyguide.md): both prefixes have to be covered, and since
> `persist.sys.` is owned by the platform (`system_prop`), the old vendor property-context
> trick no longer applies.
>
> These properties is optional to add, you can add some of it and fill it with your desired values or just leave it default "except for persist.sys.azenithIntg.state"

---

## Master switch & general

| Property | Values | Description |
| --- | --- | --- |
| `persist.sys.azenithIntg.state` | `1` = enabled, `0` = disabled | Master switch. Set it in `/vendor/build.prop` so `init.azenith.rc` starts the daemon on boot. The daemon also writes `stopped` to it while restarting the service. |
| `persist.sys.azenithIntg.disabletweak` | `1` = disable, `0` = enable | Disables every tweak without turning the daemon off. |
| `persist.sys.azenithIntg.debugmode` | `true` / `false` | Verbose (DEBUG) logging to logcat, tag `AZenith`. Restart the service after changing it. |


---

## CPU governor

| Property | Applies to |
| --- | --- |
| `persist.sys.azenithIntg.custom_default_cpu_gov` | balanced — explicit override, wins over the saved governor |
| `persist.sys.azenithIntg.custom_powersave_cpu_gov` | eco — explicit override for the powersave governor |
| `persist.sys.azenithIntg.custom_performance_cpu_gov` | performance — explicit override for the performance governor |

Resolution per profile:

- **performance / eco** — if the matching `custom_*` property is set it is applied as-is. Otherwise
  the governor currently on `cpu0` is saved and then `performance` / `powersave` is applied. This
  happens on *every* entry into the mode, so what is remembered is always the governor that was in
  place just before that switch.
- **balanced** — if `custom_default_cpu_gov` is set it is applied. Otherwise the saved governor is
  restored, falling back to leaving the kernel baseline untouched when nothing has been saved yet
  (the first apply after boot).

Every mode change re-anchors balanced to the governor the kernel was on between the two modes. A
`balanced -> performance -> balanced` cycle is a no-op for the governor; it only diverges if
something else changes the governor while a mode is active.

The saved governor lives in the daemon memory only: it is not a property and not a file, so it is
lost when the service restarts. After a restart the first `balanced` apply simply adopts whatever
governor the kernel is on.

There is no auto-detected `default_cpu_gov` property any more — the governor the daemon finds is kept
in its own state, not written to a property. Any `persist.azenith.default_cpu_gov` left over from an
older build is ignored and can be removed.

The I/O scheduler and the Mali GPU governor used to be handled the same way. Both are gone: the
daemon no longer writes to `/sys/block/*/queue/scheduler` or to any `devfreq/*.mali/governor`, so
the block scheduler keeps whatever the kernel selected and the GPU keeps its own governor. The old
`*_IO` and `*_maligpu_gov` properties are dead, and `utilityconf setsIO` / `setsMaliGov` are no
longer accepted commands.

---

## Toggles

`1` = ON, `0` = OFF. An empty value is treated as `0`.

| Property | Description |
| --- | --- |
| `persist.sys.azenithConfIntg.litemode` | Lite mode (CPU limiting path). |
| `persist.sys.azenithConfIntg.clearbg` | Clear background apps in the performance profile. |
| `persist.sys.azenithConfIntg.freqoffset` | Frequency ceiling in percent, `10`–`100`. `100` or `Disabled` = no limit. |
| `persist.sys.azenithConfIntg.fstrim` | Whether the module requests the periodic `fstrim` run. |
| `persist.sys.azenithConfIntg.logd` | Stop the logger services (`logd` and friends) while gaming. |
| `persist.sys.azenithConfIntg.walttunes` | WALT load-tracking tuning. |
| `persist.sys.azenithConfIntg.DThermal` | Custom thermal handling. |
| `persist.sys.azenithConfIntg.SFL` | SurfaceFlinger tuning. |
| `persist.sys.azenithConfIntg.malisched` | Mali scheduler tweaks. |
| `persist.sys.azenithConfIntg.fpsged` | FPSGO / GED tuning (MediaTek). |
| `persist.sys.azenithConfIntg.schedtunes` | Scheduler tuning bundle. |
| `persist.sys.azenithConfIntg.justintime` | JIT tweaks. |
| `persist.sys.azenithConfIntg.disabletrace` | Disable kernel tracing nodes. |
| `persist.sys.azenithConfIntg.usefpsgo` | `0` turns FPSGO off in the performance profile (MediaTek). |
| `persist.sys.azenithConfIntg.renderer` | HWUI/renderer backend, see below. |
| `persist.sys.azenithConfIntg.schemeconfig` | Display colour preset, see below. |

### `persist.sys.azenithConfIntg.renderer`

`default` or empty resets everything.

| Value | Effect |
| --- | --- |
| `skiavk`, `skiavkthreaded`, `vulkan` | Skia + Vulkan, `ro.hwui.use_vulkan=true` |
| `skiagl`, `skiaglthreaded`, `gles`, `opengl`, `openglthreaded` | Skia + GLES, `ro.hwui.use_vulkan=false` |
| `software` | Software renderer |
| `*threaded` suffix | also enables `debug.hwui.render_thread=true` (plus `debug.skia.threaded_mode=true` when the name contains `skia`) |

Any other value falls back to a substring check: names containing `vk`/`vulkan` get Vulkan,
names containing `gl`/`gles` get GLES.

### `persist.sys.azenithConfIntg.schemeconfig`

Display colour scheme, read once during `initialize`. It accepts a preset **name** only:

| Value | Effect |
| --- | --- |
| *(unset or empty)* | Neutral — nothing is applied. |
| `default`, `vivid`, `warm`, `cool`, `amoled` | Expanded to the preset's `R G B S` and applied. Matched case-insensitively. |
| Anything else | **Nothing is applied.** Raw slider numbers, `custom` and typos all fall here. |


| Preset | R | G | B | S |
| --- | --- | --- | --- | --- |
| `default` | 1000 | 1000 | 1000 | 1000 |
| `vivid` | 1000 | 1000 | 1000 | 1250 |
| `warm` | 1050 | 1000 | 950 | 1100 |
| `cool` | 950 | 950 | 1050 | 1000 |
| `amoled` | 1020 | 1020 | 1020 | 1150 |

The list mirrors `ColorPreset` in the AZenith app; a preset added there also has to be added to the
daemon (`Cargo/src/cmd/settings/utils/scheme.rs`) or it resolves to nothing.

---

## Info properties (not set by the Rust daemon)

For the current profile the Rust daemon writes a file, under `BASE_DIR`
(`API/current_profile` by default — see [`Config.toml`](Config.toml)).

---

## First-boot game list

There is no property for this; it is a file and a marker, both inside `GAMELIST_DIR`
(`gamelist/` by default — see [`Config.toml`](Config.toml)).

| Path | Written by | Meaning |
| --- | --- | --- |
| `gamelist/gamelistloaded` | the daemon | The device has been scanned; no further first-boot scan will run. |
| `gamelist/gamelist.txt` | the daemon (first boot), or you | One package name per line. |

On boot `initialize` checks for the marker. If it is missing, the daemon asks the
package manager (`IPackageManager.getAllPackages`) for every installed
package, writes the result to `gamelist.txt`, and only then writes the marker —
so a scan that failed (package manager not up yet, transaction refused) is retried
on the next boot instead of being lost. If the marker already exists, the daemon
only reads `gamelist.txt`; nothing is ever overwritten.

Delete `gamelist/gamelistloaded` and reboot to rebuild the list from scratch (this
throws away whatever you had in `gamelist.txt`). To add or remove single games,
edit `gamelist.txt` directly — the daemon watches it and reloads on change.

`zx.azenith` (and anything under it) is never added by the scan; the daemon and
its app are filtered out by name. The list you write by hand is never filtered.

---
