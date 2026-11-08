#!/bin/sh
#
# build_binary.sh — build the AZenith daemon (Cargo/) as an
# aarch64-linux-android binary and stage it as `vendor.azenith-service`.
#
# Usage:
#   ./build_binary.sh                 # release build
#   ./build_binary.sh --debug         # debug build
#   ./build_binary.sh --clean         # cargo clean first
#   ./build_binary.sh --rebuild-apk   # rebuild Resolver/ APK first
#   ./build_binary.sh --output DIR    # stage into DIR (default: ./output)
#   ./build_binary.sh --help
#
# The Binder Resolver APK is embedded into the binary with include_bytes!(),
# so it must exist at Cargo/binder_resolver.apk before Cargo is compiled.
# If it is missing or a Resolver source is newer, this script builds it first.
#
# Toolchains:
#   * Android NDK + cargo-ndk (CI / Linux hosts), or
#   * Termux's native aarch64 toolchain (`cargo build` directly).
#
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CARGO_DIR="$ROOT/Cargo"
TARGET="aarch64-linux-android"
PROFILE="release"
OUT_DIR="$ROOT/output"
CLEAN=0
REBUILD_APK=0
USE_NDK=0

say() {
	printf '\n==> %s\n' "$*"
}

warn() {
	printf 'warning: %s\n' "$*" >&2
}

die() {
	printf 'error: %s\n' "$*" >&2
	exit 1
}

usage() {
	sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
	case "$1" in
	--debug)
		PROFILE="debug"
		;;
	--release)
		PROFILE="release"
		;;
	--clean)
		CLEAN=1
		;;
	--rebuild-apk)
		REBUILD_APK=1
		;;
	--output)
		[ $# -ge 2 ] || die "--output needs a directory"
		OUT_DIR="$2"
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	*)
		die "unknown option: $1 (try --help)"
		;;
	esac
	shift
done

[ -f "$CARGO_DIR/Cargo.toml" ] || die "Cargo project not found at $CARGO_DIR"

# ---------------------------------------------------------------------------
# 1. Binder Resolver APK (embedded into the binary by build.rs)
# ---------------------------------------------------------------------------
APK="$CARGO_DIR/binder_resolver.apk"

# Rebuild when the APK is missing, --rebuild-apk was given, or any Resolver
# source is newer than the staged APK. Without the latter, a stale embedded APK
# would keep answering only the old questions after a source change.
apk_needs_rebuild() {
	[ "$REBUILD_APK" -eq 1 ] && return 0
	[ ! -f "$APK" ] && return 0
	for path in \
		"$ROOT/Resolver/app/src" \
		"$ROOT/Resolver/app/build.gradle.kts" \
		"$ROOT/Resolver/app/proguard-rules.pro" \
		"$ROOT/Resolver/build.gradle.kts" \
		"$ROOT/Resolver/settings.gradle.kts" \
		"$ROOT/Resolver/gradle/libs.versions.toml"; do
		[ -e "$path" ] || continue
		if [ -n "$(find "$path" -newer "$APK" -print -quit 2>/dev/null)" ]; then
			return 0
		fi
	done
	return 1
}

if apk_needs_rebuild; then
	[ -x "$ROOT/Resolver/gradlew" ] ||
		die "cannot build $APK: Resolver/gradlew is not executable"

	say "Building Binder Resolver APK"
	(
		cd "$ROOT/Resolver"
		if [ "$(uname -m)" = "aarch64" ] && command -v aapt2 >/dev/null 2>&1; then
			# Termux: the aapt2 jar bundled with AGP contains x86_64 binaries.
			./gradlew --no-daemon \
				"-Pandroid.aapt2FromMavenOverride=$(command -v aapt2)" \
				assembleRelease
		else
			./gradlew --no-daemon assembleRelease
		fi
	)

	BUILT_APK=$(find "$ROOT/Resolver/app/build/outputs/apk/release" -maxdepth 1 -type f -name '*.apk' -print -quit 2>/dev/null || true)
	[ -n "$BUILT_APK" ] || die "Binder Resolver APK was not produced under Resolver/app/build/outputs/apk/release"

	install -Dm644 "$BUILT_APK" "$APK"
fi

say "Using Binder Resolver APK: $APK ($(wc -c <"$APK") bytes)"

# ---------------------------------------------------------------------------
# 2. Pick the toolchain
# ---------------------------------------------------------------------------
HOST=$(rustc -vV | sed -n 's/^host: //p')
[ -n "$HOST" ] || die "could not determine rustc host triple"

NDK_DIR=${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-${NDK_HOME:-}}}
HAVE_SDK=0
if [ -n "${ANDROID_HOME:-}" ] || [ -n "${ANDROID_SDK_ROOT:-}" ]; then
	HAVE_SDK=1
fi

if command -v cargo-ndk >/dev/null 2>&1 && { [ -n "$NDK_DIR" ] || [ "$HAVE_SDK" -eq 1 ]; }; then
	USE_NDK=1
	say "Building with cargo-ndk (NDK: ${NDK_DIR:-auto-detected})"
else
	say "Building with the host Rust toolchain (no cargo-ndk/NDK found)"

	if command -v rustup >/dev/null 2>&1; then
		rustup target list --installed | grep -qx "$TARGET" ||
			die "Rust target $TARGET is not installed; run: rustup target add $TARGET"
	fi

	[ "$HOST" = "$TARGET" ] ||
		die "cannot target $TARGET from host $HOST without cargo-ndk/NDK; install cargo-ndk and the NDK, or build on Termux"

	# Cargo/.cargo/config.toml asks for the API-34 clang wrapper. Termux ships
	# an API-24 wrapper instead; use it for local builds when the exact one is
	# unavailable (the daemon still targets Android 34+ at runtime).
	if ! command -v aarch64-linux-android34-clang >/dev/null 2>&1; then
		if command -v aarch64-linux-android-clang >/dev/null 2>&1; then
			warn "aarch64-linux-android34-clang not found; falling back to aarch64-linux-android-clang (Termux)"
			CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android-clang
			export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
		else
			die "no Android clang linker found; install the NDK or Termux's aarch64-linux-android-clang"
		fi
	fi
fi

# ---------------------------------------------------------------------------
# 3. Build
# ---------------------------------------------------------------------------
cd "$CARGO_DIR"

if [ "$CLEAN" -eq 1 ]; then
	say "Cleaning previous build output"
	cargo clean
fi

say "Building azenith-service ($PROFILE)"
if [ "$USE_NDK" -eq 1 ]; then
	if [ "$PROFILE" = "release" ]; then
		cargo ndk -t arm64-v8a build --release
	else
		cargo ndk -t arm64-v8a build
	fi
else
	if [ "$PROFILE" = "release" ]; then
		cargo build --release
	else
		cargo build
	fi
fi

BINARY="$CARGO_DIR/target/$TARGET/$PROFILE/azenith-service"
[ -f "$BINARY" ] || die "build finished but binary is missing: $BINARY"

# ---------------------------------------------------------------------------
# 4. Stage
# ---------------------------------------------------------------------------
OUT="$OUT_DIR/vendor.azenith-service"
install -Dm755 "$BINARY" "$OUT"

say "Done"
printf 'Binary: %s\n' "$OUT"
ls -lh "$OUT"
if command -v file >/dev/null 2>&1; then
	file "$OUT"
fi
