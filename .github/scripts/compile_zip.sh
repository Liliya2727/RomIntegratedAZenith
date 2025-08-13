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
cp -r ./libs modules/hw
cp -r ./init.azenith.rc modules/init
cp -r ./tweakfls/* modules/tweakfile
cp LICENSE ./modules

# Remove .sh extension from scripts
find modules/hw -maxdepth 1 -type f -name "*.sh" -exec sh -c 'mv -- "$0" "${0%.sh}"' {} \;

# Parse version info to module prop
zipName="AZenithRomINT$version-$release_code"
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
