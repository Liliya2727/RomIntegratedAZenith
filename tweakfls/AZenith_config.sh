#!/vendor/bin/sh

#
# Copyright (C) 2024-2025 Zexshia
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

# Wait until boot is completed and /sdcard mounted
while true; do
    boot_completed=$(getprop sys.boot_completed)
    
    if [ "$boot_completed" = "1" ] && [ -d /sdcard ]; then
        break
    fi
    
    sleep 1
done

# Make dir and files before writing
mkdir -p /sdcard/config/
touch /sdcard/config/AZenithDefaultGov
touch /sdcard/config/soctype
touch /sdcard/config/current_profile
touch /sdcard/config/gameinfo
touch /sdcard/config/clearbg
touch /sdcard/gamelist.txt

# Add for pre-added packages
gamelist_flag=$(getprop persist.sys.gamelisted)
if [ -z "$gamelist_flag" ] || [ "$gamelist_flag" = "0" ]; then
    echo "com.mobile.legends" >> /sdcard/gamelist.txt
    echo "com.HoYoverse.Nap" >> /sdcard/gamelist.txt
    echo "com.HoYoverse.hkrpgoversea" >> /sdcard/gamelist.txt
    echo "com.YoStarEN.Arknights" >> /sdcard/gamelist.txt
    echo "com.YoStarEN.HBR" >> /sdcard/gamelist.txt
    echo "com.YoStarEN.MahjongSoul" >> /sdcard/gamelist.txt
    echo "com.YoStarJP.MajSoul" >> /sdcard/gamelist.txt
    echo "com.YoStar.AetherGazer" >> /sdcard/gamelist.txt
    echo "com.YostarJP.BlueArchive" >> /sdcard/gamelist.txt

    setprop persist.sys.gamelisted 1
fi

# start azenith daemon
setprop sys.azenith.config ready