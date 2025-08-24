#!/bin/env bash

if [ -z "$GITHUB_WORKSPACE" ]; then
	echo "This script should only run on GitHub action!" >&2
	exit 1
fi

# Make sure we're on right directory
cd "$GITHUB_WORKSPACE" || {
	echo "Unable to cd to GITHUB_WORKSPACE" >&2
	exit 1
}

# Version info
version="$(cat version)"
version_code="$(git rev-list HEAD --count)"
release_code="$(git rev-parse --short HEAD)-Release"

# Copy module files
mkdir -p modules/bin/
mkdir -p modules/bin/hw
mkdir -p modules/init
cp -r ./libs/arm64-v8a/vendor.azenith-service modules/bin/hw
cp -r ./init.azenith.rc modules/init
cp -r ./tweakfls/* modules/bin
cp -r ./preloadbin/* modules/bin
cp -r ./vendor_sepolicy.cil modules/
cp LICENSE ./modules
cp gamelist.txt ./modules
cp README.md ./modules
cp changelog.md ./modules
cp listproperties.txt ./modules

# Remove .sh extension from scripts
find modules/bin -maxdepth 1 -type f -name "*.sh" -exec sh -c 'mv -- "$0" "${0%.sh}"' {} \;

# Parse version info to module prop
zipName="AZenithRomINT-v$version-$release_code"
echo "zipName=$zipName" >>"$GITHUB_OUTPUT"

# Zip the file
cd ./modules || {
	echo "Unable to cd to ./modules" >&2
	exit 1
}

zip -r9 ../"$zipName" * -x *placeholder* *.map .shellcheckrc
zip -z ../"$zipName" <<EOF
$version-$release_code
Build Date $(date +"%a %b %d %H:%M:%S %Z %Y")
EOF
