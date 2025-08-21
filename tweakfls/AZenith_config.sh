#!/vendor/bin/sh

#
# Copyright (C) 2024-2025 Rianixia
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# export full path to ensure ts runs
export PATH="/product/bin:/apex/com.android.runtime/bin:/apex/com.android.art/bin:/system_ext/bin:/system/bin:/system/xbin:/odm/bin:/vendor/bin:/vendor/xbin"


DEBUG_LOG=$(getprop persist.sys.azenith-debug)

AZLog() {
    if [ "$DEBUG_LOG" = "true" ]; then
        log -p i -t "AZenith" "$1"
    fi
}

AZError() {
    log -p e -t "AZenith" "$1"
}

on_exit() {
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        AZLog "AZenith init script finished successfully."
    else
        AZError "AZenith init script crashed with exit code $exit_code."
    fi
}
trap on_exit EXIT

AZLog "Starting AZenith config init script..."

# Wait until boot is completed and /sdcard mounted
while true; do
    boot_completed=$(getprop sys.boot_completed)
    if [ "$boot_completed" = "1" ] && [ -d /sdcard ]; then
        AZLog "Boot complete and /sdcard detected."
        break
    fi
    sleep 1
done

# Make dir and files before writing
AZLog "Creating /sdcard/config directory and default files..."
if mkdir -p /sdcard/config/; then
    AZLog "Directory /sdcard/config created successfully."
else
    AZError "Failed to create /sdcard/config directory!"
    exit 1
fi

for file in AZenithDefaultGov; do
    if touch /sdcard/config/$file; then
        AZLog "File /sdcard/config/$file created."
    else
        AZError "Failed to create /sdcard/config/$file!"
        exit 1
    fi
done

if touch /sdcard/gamelist.txt; then
    AZLog "File /sdcard/gamelist.txt created."
else
    AZError "Failed to create /sdcard/gamelist.txt!"
    exit 1
fi

# Add for pre-added packages (only if not already set)
gamelist_flag=$(getprop persist.sys.gamelisted)
AZLog "Current gamelist flag: $gamelist_flag"

if [ -z "$gamelist_flag" ] || [ "$gamelist_flag" = "0" ]; then
    AZLog "Writing default game list..."
    {
        echo "com.mobile.legends"
        echo "com.HoYoverse.Nap"
        echo "com.HoYoverse.hkrpgoversea"
        echo "com.YoStarEN.Arknights"
        echo "com.YoStarEN.HBR"
        echo "com.YoStarEN.MahjongSoul"
        echo "com.YoStarJP.MajSoul"
        echo "com.YoStar.AetherGazer"
        echo "com.YostarJP.BlueArchive"
    } > /sdcard/gamelist.txt

    if [ $? -eq 0 ]; then
        AZLog "Default gamelist written successfully."
        setprop persist.sys.gamelisted 1
        AZLog "persist.sys.gamelisted set to 1."
    else
        AZError "Failed to write default gamelist!"
        exit 1
    fi
else
    AZLog "Gamelist already initialized. Skipping..."
fi

if setprop sys.azenith.config ready; then
    AZLog "sys.azenith.config set to ready."
else
    AZError "Failed to set sys.azenith.config property!"
    exit 1
fi