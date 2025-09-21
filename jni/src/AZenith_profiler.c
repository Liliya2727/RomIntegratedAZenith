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
#include <unistd.h> // For sleep()
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
            case 0: final_mode = 3; break; // Powersave
            case 1: final_mode = 2; break; // Balance
            case 2: final_mode = 1; break; // Performance
            default: final_mode = -1; break; // Unknown
        }
        free(mode_str);
    } else {
        log_zenith(LOG_WARN, "Failed to read Transsion Game Mode setting.");
    }
    return final_mode;
}

/**
 * @brief Applies the specified performance profile by setting system properties.
 *
 * @param profile The profile to apply (1: Perf, 2: Normal, 3: Powersave).
 */
static void apply_profile(int profile) {
    systemv("/vendor/bin/setprop sys.azenith.currentprofile %d", profile);
    systemv("/vendor/bin/AZenith_Profiler %d", profile);
    log_zenith(LOG_INFO, "Successfully applied profile: %d", profile);
}

 /***********************************************************************************
 * Function Name      : sync_game_profile_loop
 * Inputs             : get_transsion_game_mode
 * Description        : This function runs as long as a game is active. It polls the Game Space
 *                      mode every 2 seconds and applies the corresponding profile if it changes.
 *                      When the game exits, the loop terminates and the profile is reverted to normal.
 ***********************************************************************************/
static void sync_game_profile_loop(void) {
    int last_known_profile = -1;
    char* current_game = get_gamestart();

    // Loop as long as a game is detected in the foreground
    while (current_game != NULL && strlen(current_game) > 0) {
        int transsion_mode = get_transsion_game_mode();

        // Check if the mode is valid and has changed since the last check
        if (transsion_mode != -1 && transsion_mode != last_known_profile) {
            log_zenith(LOG_INFO, "Transsion Game Mode changed to %d. Syncing...", transsion_mode);
            apply_profile(transsion_mode);
            last_known_profile = transsion_mode;
        }

        // Wait for 2 seconds before the next check to avoid high CPU usage
        sleep(2);

        // Check again if the game is still running
        free(current_game); // Free memory from the previous check
        current_game = get_gamestart();
    }
    
    // Cleanup after the loop finishes
    if (current_game) {
        free(current_game);
    }

    log_zenith(LOG_INFO, "Game has exited. Reverting to normal profile.");
    systemv("/vendor/bin/setprop sys.azenith.gameinfo \"NULL 0 0\"");
    apply_profile(2); // Revert to profile 2 (Normal)
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
    // Perform a one-time check for Transsion Game Space support
    if (!gamespace_props_checked) {
        char* support_prop = execute_command("/system/bin/getprop persist.sys.azenith.syncgamespace.support");
        char* is_transsion_prop = execute_command("/system/bin/getprop ro.product.brand");

        if (support_prop && is_transsion_prop &&
            strncmp(support_prop, "1", 1) == 0 &&
            (strncmp(is_transsion_prop, "TECNO", 5) == 0 || strncmp(is_transsion_prop, "Infinix", 7) == 0 || strncmp(is_transsion_prop, "itel", 4) == 0)) {
            transsion_gamespace_support = true;
            log_zenith(LOG_INFO, "Transsion Game Space sync is supported and enabled.");
        }

        if (support_prop) free(support_prop);
        if (is_transsion_prop) free(is_transsion_prop);
        gamespace_props_checked = true;
    }

    // A game has been launched
    if (profile == 1) {
        char gameinfo_prop[256];
        snprintf(gameinfo_prop, sizeof(gameinfo_prop), "%s %d %d", gamestart, game_pid, uidof(game_pid));
        systemv("/vendor/bin/setprop sys.azenith.gameinfo \"%s\"", gameinfo_prop);

        if (transsion_gamespace_support) {
            // Enter the real-time sync loop. This function will block and handle
            // profile changes until the game exits.
            log_zenith(LOG_INFO, "Game detected. Starting real-time Transsion Game Space sync.");
            sync_game_profile_loop();
        } else {
            // Standard behavior: apply performance profile and exit
            log_zenith(LOG_INFO, "Game detected. Applying default performance profile.");
            apply_profile(1);
        }
    } else {
        // A non-game profile is requested (e.g., normal, powersave)
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
 ***********************************************************************************/
char* get_gamestart(void) {
    return execute_command("/system/bin/dumpsys window visible-apps | /vendor/bin/grep 'package=.* ' | /vendor/bin/grep -Eo -f %s",
                           GAMELIST);
}
/***********************************************************************************
 * Function Name      : get_screenstate_normal
 * Inputs             : None
 * Returns            : bool - true if screen was awake
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
