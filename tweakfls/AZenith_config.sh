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

# Wait until boot is completed and /sdcard is fully writable
while true; do
    boot_completed=$(getprop sys.boot_completed)
    # Also check if we can actually write to the directory
    if [ "$boot_completed" = "1" ] && [ -d /sdcard ] && touch /sdcard/.tmp_azenith_check 2>/dev/null; then
        # If the check file was created successfully, remove it and exit the loop
        rm /sdcard/.tmp_azenith_check
        AZLog "Boot complete and /sdcard is writable."
        break
    fi
    sleep 1
done

if touch /sdcard/gamelist.txt; then
    AZLog "File /sdcard/gamelist.txt created."
else
    AZError "Failed to create /sdcard/gamelist.txt!"
    exit 1
fi

# Add all prelisted games
gamelist_flag=$(getprop persist.sys.gamelisted)
AZLog "Current gamelist flag: $gamelist_flag"
if [ -z "$gamelist_flag" ] || [ "$gamelist_flag" = "0" ]; then
    AZLog "Writing default game list..."
    cat <<EOF > /sdcard/gamelist.txt
com.proximabeta.mf.uamo
com.dts.freefiremax
com.dts.freefireth
com.levelinfinite.sgameGlobal
com.tencent.KiHan
com.tencent.tmgp.cf
com.tencent.tmgp.cod
com.tencent.tmgp.gnyx
com.delta.force.hawk.ops
com.garena.game.df
com.levelinfinite.hotta.gp
com.supercell.clashofclans
com.mobile.legends
com.vng.mlbbvn
com.tencent.tmgp.sgame
com.YoStar.AetherGazer
com.netease.lztgglobal
com.riotgames.league.wildrift
com.riotgames.league.wildrifttw
com.riotgames.league.wildriftvn
com.epicgames.fortnite
com.epicgames.portal
com.tencent.lolm
jp.konami.pesam
com.cygames.umaumusume
com.ea.gp.fifamobile
com.pearlabyss.blackdesertm.gl
com.pearlabyss.blackdesertm
com.activision.callofduty.shooter
com.gameloft.android.ANMP.GloftA9HM
com.madfingergames.legends
com.riotgames.league.teamfighttactics
com.riotgames.league.teamfighttacticstw
com.riotgames.league.teamfighttacticsvn
com.pubg.imobile
com.pubg.krmobile
com.rekoo.pubgm
com.tencent.tmgp.pubgmhd
com.vng.pubgmobile
com.tencent.ig
com.garena.game.codm
com.tencent.tmgp.kr.codm
com.vng.codmvn
com.miraclegames.farlight84
EOF

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
