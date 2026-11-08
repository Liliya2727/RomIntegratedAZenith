# AZenith for Developers

Rom Integrated AZenith — system-level performance daemon for custom ROMs.
Built by Rom Maintainers, for Rom Maintainers.

Developed by [@Zexshia](https://t.me/Zexshia) and [@rianixia](https://t.me/rianixia) on Telegram.

---

## Quick Start

There is no CI build pipeline in this repo — every maintainer builds the binary themselves.

### 1. Clone or fork the repository

```sh
git clone https://github.com/Liliya2727/RomIntegratedAZenith.git
cd RomIntegratedAZenith
```

### 2. Configure `Config.toml`

Edit [`Config.toml`](Config.toml) **before building** — see the [Build Configuration](#build-configuration-configtoml) section for the full reference.

### 3. Build

```sh
./build_binary.sh             # release build
./build_binary.sh --debug     # debug build
./build_binary.sh --clean     # clean target first
./build_binary.sh --output /path/to/staging
```

`build_binary.sh` automatically:
- builds the Binder Resolver APK (embedded into the binary via `include_bytes!`),
- picks up your `Config.toml`,
- compiles `azenith-service` for `aarch64-linux-android`,
- stages the result at `output/vendor.azenith-service`.

That's it — `output/vendor.azenith-service` is the binary to drop into your vendor image.

---

## How to Add AZenith into my Vendor?

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

---

## Credits

- [@Kombat](https://t.me/Kombat)
- [@Kaminarich](https://t.me/Kaminarich)
- [@Rem01Gaming](https://t.me/Rem01Gaming)

---

## Full Docs

See [DOCS.md](DOCS.md) for the complete technical reference (architecture, SELinux policy, system properties, file layout, and more).
