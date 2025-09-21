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
#include <errno.h>  // For errno
#include <string.h> // For strerror()
/* add path access for full path*/
#include <stdlib.h>

// Define binary path needed for get_transsion_game_mode
#define SETTINGS_PATH "/system/bin/settings"

void setup_path(void) {
    int result = setenv("PATH",
                        "/product/bin:/apex/com.android.runtime/bin:/apex/com.android.art/bin:"
                        "/system_ext/bin:/system/bin:/system/xbin:/odm/bin:/vendor/bin:/vendor/xbin",
                        1 /* overwrite existing value */
    );

    // Check the return value of setenv
    if (result == 0) {
        log_zenith(LOG_INFO, "PATH environment variable set successfully.");
    } else {
        // If it fails, log the specific error from the system
        log_zenith(LOG_ERROR, "Failed to set PATH environment variable: %s", strerror(errno));
    }
}

bool (*get_screenstate)(void) = get_screenstate_normal;
bool (*get_low_power_state)(void) = get_low_power_state_normal;

// For Transsion Game Space Sync
static bool transsion_gamespace_support = false;
static bool gamespace_props_checked = false;

// Get game mode from Transsion's Game Space
static int get_transsion_game_mode(void) {
    char* mode_str = execute_direct(SETTINGS_PATH, "settings", "get", "secure", "transsion_game_function_mode", NULL);
    int final_mode = -1;

    if (mode_str) {
        int mode = -1;
        if (strlen(mode_str) > 0 && strcmp(mode_str, "null") != 0) {
            mode = atoi(mode_str);
        }

        switch (mode) {
            case 0: // Powersave
                log_zenith(LOG_INFO, "Transsion Game Mode: Powersave");
                final_mode = 3;
                break;
            case 1: // Balance
                log_zenith(LOG_INFO, "Transsion Game Mode: Balance");
                final_mode = 2;
                break;
            case 2: // Performance
                log_zenith(LOG_INFO, "Transsion Game Mode: Performance");
                final_mode = 1;
                break;
            default:
                log_zenith(LOG_WARN, "Unknown Transsion Game Mode value received.");
                final_mode = -1;
                break;
        }
        free(mode_str);
    } else {
        log_zenith(LOG_WARN, "Failed to read Transsion Game Mode setting.");
    }

    return final_mode;
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
    int final_profile = profile;

    // Check for Transsion Game Space support, but only once.
    if (!gamespace_props_checked) {
        char* support_prop = execute_command("/system/bin/getprop persist.sys.azenith.syncgamespace.support");
        char* is_transsion_prop = execute_command("/system/bin/getprop persist.sys.azenith.issupportgamespace");

        // Use strncmp to safely compare prop values without needing to trim newlines
        if (support_prop && is_transsion_prop &&
            strncmp(support_prop, "1", 1) == 0 &&
            strncmp(is_transsion_prop, "transsion", 9) == 0) {
            transsion_gamespace_support = true;
            log_zenith(LOG_INFO, "Transsion Game Space support detected and enabled.");
        }

        if (support_prop) free(support_prop);
        if (is_transsion_prop) free(is_transsion_prop);
        gamespace_props_checked = true;
    }

    // If game profile is requested and Transsion support is enabled, sync with its game space setting.
    if (profile == 1 && transsion_gamespace_support) {
        int transsion_profile = get_transsion_game_mode();
        if (transsion_profile != -1) {
            log_zenith(LOG_INFO, "Syncing with Transsion Game Space. Setting profile to %d", transsion_profile);
            final_profile = transsion_profile;
        } else {
            log_zenith(LOG_WARN, "Failed to get Transsion Game Space mode, using default performance profile.");
        }
    }

    char gameinfo_prop[256];
    if (profile == 1) {
        snprintf(gameinfo_prop, sizeof(gameinfo_prop), "%s %d %d", gamestart, game_pid, uidof(game_pid));
        systemv("/vendor/bin/setprop sys.azenith.gameinfo \"%s\"", gameinfo_prop);
    } else {
        systemv("/vendor/bin/setprop sys.azenith.gameinfo \"NULL 0 0\"");
    }
    systemv("/vendor/bin/setprop sys.azenith.currentprofile %d", final_profile);
    systemv("/vendor/bin/AZenith_Profiler %d", final_profile);
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
    return execute_command("/system/bin/dumpsys window visible-apps | /vendor/bin/grep 'package=.* ' | /vendor/bin/grep -Eo -f %s",
                           GAMELIST);
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

        // Set default state after too many failures via function pointer
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

        // Set default state after too many failures via function pointer
        get_low_power_state = return_false;
    }

    return false;
}