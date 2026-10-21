# AZenith for Developers
Introducing Rom Integrated AZenith for developers/Rom Maintainer,
Developed By @Zexshia and @rianixia on telegram

Download from [Github Releases](https://github.com/Liliya2727/RomIntegratedAZenith/releases)

## How to Add AZenith into my vendor?
1. Place vendor.azenith-service binary to /vendor/bin/hw/
2. Place init.azenith.rc to /vendor/etc/init/
3. Place AZenith_Profiler and AZenith_config to /vendor/bin/
4. Patch vendor [sepolicy](sepolicyguide.md)
5. In /vendor/build.prop add "persist.azenith.state" prop value 1 for enabled, and value 0 for disabled
6. To enable other tweaks and customization refer to the [properties list](listproperties.md)
7. Repack Vendor

## Build configuration (Config.toml)
By default every runtime file AZenith creates lives under `/data/adb/.config/AntaresRS/`.
If your ROM layout needs a different place, edit [`Config.toml`](Config.toml) **before building**
and recompile — no path is hardcoded in the Rust sources anymore.

```toml
[PATHS]
BASE_DIR      = "/data/adb/azenith"   # everything below follows this
GAMELIST_DIR  = "/sdcard/AZ-games"    # absolute path = overrides just this entry
```

Keys are uppercase and named exactly like the Rust constant they generate, so
`BASE_DIR = "..."` becomes `pub(crate) const BASE_DIR: &str = "..."`.

Rules:
- Value starting with `/` is used as-is.
- Any other value is relative to its parent (`BASE_DIR`, `GAMELIST_DIR` or `API_DIR`).
- An empty string means "use the parent path".

That means changing `BASE_DIR` alone relocates the whole tree (`state.toml`, `gamelist/`,
`API/`, the binder resolver cache, the instance lock), and files declared relative to a directory
you overrode keep following it. `Cargo/build.rs` resolves everything at compile time and generates
`$OUT_DIR/paths.rs`; an invalid (non-absolute) path falls back to the default and prints a
`cargo:warning`. Delete any key you do not care about — the defaults are applied automatically.

### Choosing your chipset

The SoC family is also baked in at build time (it used to be the runtime property
`persist.azenith.soctype`), so set it once before building:

```toml
[DEVICE]
SOC = "mediatek"   # mediatek | snapdragon | exynos | unisoc | tensor | none
```

One binary carries exactly one chipset set, so `none` simply means "only the generic tweaks".
An unknown value falls back to `none` with a build warning; the old numeric values (`1`–`5`)
are still accepted and normalized to the names above.

## Logging
AZenith does not write any log file. Everything goes to logcat under the `AZenith` tag:

```
adb logcat -s AZenith
```

Each line is prefixed with the component that emitted it, e.g. `[daemon.profiler]` or
`[cmd.settings.cpu]`.

Verbose (DEBUG) output is opt-in through a single property, no flag file involved:

```
setprop persist.azenith.debugmode true   # enable DEBUG
setprop persist.azenith.debugmode false  # back to info
```

Restart the service after flipping it. For a one-off run of a companion command you can also
override the level with the `LIBBINDING_LOG` environment variable
(`debug` / `info` / `warn` / `error`).

## How add your app packages?
1. Create the file gamelist.txt on /sdcard/gamelist.txt (If AZenith is properly running it should generate an example gamelist.txt)
2. Add your package inside the file for example "coample gamem.mobile.legends" if you want more package just add them in new lines
For Example
```
com.mobile.legends
adventure.rpg.anime.game.vng.ys6
age.of.civilizations2.jakowski.lukasz
air.com.ace2three.mobile.cash
(and so on)
```
3. That's it! Enjoy

# Credits
- @Kombat
- @Kaminarich
- @Rem01Gaming
