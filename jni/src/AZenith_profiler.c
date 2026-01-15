/*
 * Copyright (C) 2024-2025 Rem01Gaming
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
#include <AZenith.h>
#include <errno.h>  
#include <string.h> 
#include <stdlib.h>
#include <unistd.h> 

static void apply_profile(int profile);
char* get_gamestart(void);

void setup_path(void) {
    int result = setenv("PATH",
                        "/product/bin:/apex/com.android.runtime/bin:/apex/com.android.art/bin:"
                        "/system_ext/bin:/system/bin:/system/xbin:/odm/bin:/vendor/bin:/vendor/xbin",
                        1 /* overwrite existing value */
    );

    if (result == 0) {
        log_zenith(LOG_INFO, "PATH environment variable set successfully.");
    } else {
        log_zenith(LOG_ERROR, "Failed to set PATH environment variable: %s", strerror(errno));
    }
}

bool (*get_screenstate)(void) = get_screenstate_normal;
bool (*get_low_power_state)(void) = get_low_power_state_normal;

/***********************************************************************************
 * Function Name      : apply_profile
 * Inputs             : int profile
 * Returns            : None
 * Description        : Applies the specified performance profile.
 ***********************************************************************************/
static void apply_profile(int profile) {
    systemv("/vendor/bin/setprop sys.azenith.currentprofile %d", profile);
    systemv("/vendor/bin/AZenith_Profiler %d", profile);
    log_zenith(LOG_INFO, "Successfully applied profile: %d", profile);
}

/***********************************************************************************
 * Function Name      : run_profiler
 * Inputs             : int - 0 for perfcommon
 * 1 for performance
 * 2 for normal
 * 3 for powersave
 * Returns            : None
 * Description        : Switch to specified performance profile.
 ***********************************************************************************/
void run_profiler(const int profile) {
    if (profile == 1) {
        // A game has been launched.
        char gameinfo_prop[256];
        snprintf(gameinfo_prop, sizeof(gameinfo_prop), "%s %d %d", gamestart, game_pid, uidof(game_pid));
        systemv("/vendor/bin/setprop sys.azenith.gameinfo \"%s\"", gameinfo_prop);

        log_zenith(LOG_INFO, "Game detected. Applying default performance profile.");
        apply_profile(1);
    } else {
        // A non-game profile is requested (e.g., normal, powersave).
        systemv("/vendor/bin/setprop sys.azenith.gameinfo \"NULL 0 0\"");
        apply_profile(profile);
    }
}

/***********************************************************************************
 * Function Name      : get_gamestart
 * Inputs             : None
 * Returns            : char* (dynamically allocated string with the game package name)
 * Description        : Searches for the currently visible application that matches
 * any package name listed in gamelist.
 * This helps identify if a specific game is running in the foreground.
 * Uses dumpsys to retrieve visible apps and filters by packages
 * listed in Gamelist.
 * Note               : Caller is responsible for freeing the returned string.
 ***********************************************************************************/
char* get_gamestart(void) {
    char* list_path = get_gamelist_path();
    return execute_command("/system/bin/dumpsys window visible-apps | /vendor/bin/grep 'package=.* ' | /vendor/bin/grep -Eo -f %s",
                           list_path);
}

/***********************************************************************************
 * Function Name      : get_screenstate_normal
 * Inputs             : None
 * Returns            : bool - true if screen was awake
 * false if screen was asleep
 * Description        : Retrieves the current screen wakefulness state from dumpsys command.
 * Note               : In repeated failures up to 6, this function will skip fetch routine
 * and just return true all time using function pointer.
 * Never call this function, call get_screenstate() instead.
 ***********************************************************************************/
bool get_screenstate_normal(void) {
    static char fetch_failed = 0;

    char* screenstate = execute_command("/system/bin/dumpsys power | /vendor/bin/grep -Eo 'mWakefulness=Awake|mWakefulness=Asleep' "
                                        "| /system/bin/awk -F'=' '{print $2}'");

    if (screenstate) [[clang::likely]] {
        fetch_failed = 0;
        return IS_AWAKE(screenstate);
    }

    fetch_failed++;
    log_zenith(LOG_ERROR, "Unable to fetch current screenstate");

    if (fetch_failed == 6) {
        log_zenith(LOG_FATAL, "get_screenstate is out of order!");
        get_screenstate = return_true;
    }

    return true;
}

/***********************************************************************************
 * Function Name      : get_low_power_state_normal
 * Inputs             : None
 * Returns            : bool - true if Battery Saver is enabled
 * false otherwise
 * Description        : Checks if the device's Battery Saver mode is enabled by using
 * global db or dumpsys power.
 * Note               : In repeated failures up to 6, this function will skip fetch routine
 * and just return false all time using function pointer.
 * Never call this function, call get_low_power_state() instead.
 ***********************************************************************************/
bool get_low_power_state_normal(void) {
    static char fetch_failed = 0;

    char* low_power = execute_direct("/system/bin/settings", "settings", "get", "global", "low_power", NULL);
    if (!low_power) {
        low_power = execute_command("/system/bin/dumpsys power | /vendor/bin/grep -Eo "
                                    "'mSettingBatterySaverEnabled=true|mSettingBatterySaverEnabled=false' | "
                                    "/system/bin/awk -F'=' '{print $2}'");
    }

    if (low_power) [[clang::likely]] {
        fetch_failed = 0;
        return IS_LOW_POWER(low_power);
    }

    fetch_failed++;
    log_zenith(LOG_ERROR, "Unable to fetch battery saver status");

    if (fetch_failed == 6) {
        log_zenith(LOG_FATAL, "get_low_power_state is out of order!");
        get_low_power_state = return_false;
    }

    return false;
}
