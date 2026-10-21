#!/bin/env bash
#
# Assemble the AZenith package zip from the artifacts produced by the
# previous workflow steps:
#
#   Resolver  -> Binder Resolver APK (staged as Cargo/binder_resolver.apk)
#   Cargo     -> Cargo/target/aarch64-linux-android/release/sys_azenithrs-service
#
# The tree staged here is what a ROM integrator extracts; file names match
# what init.azenith.rc and Config.toml expect at runtime.

set -euo pipefail

if [ -z "${GITHUB_WORKSPACE:-}" ]; then
	echo "This script should only run on GitHub action!" >&2
	exit 1
fi

# Make sure we're on right directory
cd "$GITHUB_WORKSPACE" || {
	echo "Unable to cd to GITHUB_WORKSPACE" >&2
	exit 1
}

# ---- inputs built by earlier steps -------------------------------------
BINARY="Cargo/target/aarch64-linux-android/release/sys_azenithrs-service"
APK="Cargo/binder_resolver.apk"

if [ ! -f "$BINARY" ]; then
	echo "Rust binary not found: $BINARY (did 'cargo ndk build --release' run?)" >&2
	exit 1
fi
if [ ! -f "$APK" ]; then
	echo "Binder Resolver APK not found: $APK (did the gradle step run?)" >&2
	exit 1
fi

# Version info
version="$(cat version)"
version_code="$(git rev-list HEAD --count)"
release_code="$(git rev-parse --short HEAD)-Release"

# ---- stage the package tree --------------------------------------------
rm -rf modules extracted
mkdir -p modules/bin/hw modules/init modules/gamelist

# /vendor/bin/hw/vendor.azenith-service   (see init.azenith.rc: `service AZenith /vendor/bin/hw/vendor.azenith-service`)
install -Dm755 "$BINARY" modules/bin/hw/vendor.azenith-service

# /vendor/etc/init/init.azenith.rc
install -Dm644 init.azenith.rc modules/init/init.azenith.rc

# Runtime files -> land under BASE_DIR (see Config.toml). binder_resolver.apk
# is also embedded in the binary, but the daemon rewrites it at runtime, so it
# ships as a plain file too.
install -Dm644 "$APK" modules/binder_resolver.apk
install -Dm644 gamelist.txt modules/gamelist/gamelist.txt

# SELinux policy + docs for the integrator
install -Dm644 vendor_sepolicy.cil modules/vendor_sepolicy.cil
install -Dm644 azenith_sepolicy.cil modules/azenith_sepolicy.cil
install -Dm644 LICENSE modules/LICENSE
install -Dm644 README.md modules/README.md
install -Dm644 changelog.md modules/changelog.md
install -Dm644 listproperties.md modules/listproperties.md
install -Dm644 sepolicyguide.md modules/sepolicyguide.md

# Record the build configuration that was baked into the binary
install -Dm644 Config.toml modules/Config.toml

# Remove .sh extension from helper scripts, if any get added to modules/bin
find modules/bin -maxdepth 1 -type f -name "*.sh" -exec sh -c 'mv -- "$0" "${0%.sh}"' {} \;

echo "Staged tree:"
find modules -type f | sort

# Parse version info to module prop
zipName="AZenithRomINT-v$version-$release_code.zip"
echo "zipName=$zipName" >>"$GITHUB_OUTPUT"

# Zip the file
cd ./modules || {
	echo "Unable to cd to ./modules" >&2
	exit 1
}

zip -r9 ../"$zipName" * -x *placeholder* *.map .shellcheckrc
zip -z ../"$zipName" <<EOF
$version-$release_code (build $version_code)
Build Date $(date +"%a %b %d %H:%M:%S %Z %Y")
EOF
