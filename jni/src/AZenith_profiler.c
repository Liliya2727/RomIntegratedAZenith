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
#include <unistd.h> // Required for fork(), sleep(), _exit()

// Define binary path needed for get_transsion_game_mode
#define SETTINGS_PATH "/system/bin/settings"

// Forward declarations
static void apply_profile(int profile);
char* get_gamestart(void);

// Global function pointers
bool (*get_screenstate)(void) = get_screenstate_normal;
bool (*get_low_power_state)(void) = get_low_power_state_normal;

// Static variables for Transsion Game Space Sync
static bool transsion_gamespace_support = false;
static bool gamespace_props_checked = false;

void setup_path(void) {
    int result = setenv("PATH",
                        "/product/bin:/apex/com.android.runtime/bin:/apex/com.android.art/bin:"
                        "/system_ext/bin:/system/bin:/system/xbin:/odm/bin:/vendor/bin:/vendor/xbin",
                        1 /* overwrite */);
    if (result != 0) {
        log_zenith(LOG_ERROR, "Failed to set PATH environment variable: %s", strerror(errno));
    }
}

// Get game mode from Transsion's Game Space (Unchanged)
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
 * applies the specified performance profile.
 */
static void apply_profile(int profile) {
    systemv("/vendor/bin/setprop sys.azenith.currentprofile %d", profile);
    systemv("/vendor/bin/AZenith_Profiler %d", profile);
    log_zenith(LOG_INFO, "Successfully applied profile: %d", profile);
}

/**
 * The monitoring loop that runs in a separate process for a game session.
 */
static void sync_game_profile_loop(void) {
    int last_known_profile = -1;
    char* current_game = get_gamestart();

    // Loop as long as a game is detected in the foreground
    while (current_game != NULL && strlen(current_game) > 0) {
        int transsion_mode = get_transsion_game_mode();

        if (transsion_mode != -1 && transsion_mode != last_known_profile) {
            log_zenith(LOG_INFO, "Transsion Game Mode changed to %d. Syncing...", transsion_mode);
            apply_profile(transsion_mode);
            last_known_profile = transsion_mode;
        }

        sleep(2); // Wait before the next check

        free(current_game); // Free memory from the previous check
        current_game = get_gamestart();
    }
    
    // Cleanup after the loop finishes (game is no longer in foreground)
    if (current_game) {
        free(current_game);
    }

    log_zenith(LOG_INFO, "Game has exited foreground. Reverting to normal profile.");
    systemv("/vendor/bin/setprop sys.azenith.gameinfo \"NULL 0 0\"");
    apply_profile(2); // Revert to profile 2 (Normal)
}

void run_profiler(const int profile) {
    // Perform a one-time check for Transsion Game Space support
    if (!gamespace_props_checked) {
        char* support_prop = execute_command("/system/bin/getprop persist.sys.azenith.syncgamespace.support");
        char* is_transsion_prop = execute_command("/system/bin/getprop persist.sys.azenith.issupportgamespace");

        if (support_prop && is_transsion_prop &&
            strncmp(support_prop, "1", 1) == 0 &&
            strncmp(is_transsion_prop, "transsion", 9) == 0) {
            transsion_gamespace_support = true;
            log_zenith(LOG_INFO, "Transsion Game Space sync is supported and enabled.");
        }

        if (support_prop) free(support_prop);
        if (is_transsion_prop) free(is_transsion_prop);
        gamespace_props_checked = true;
    }

    if (profile == 1) {
        // A game has been launched. Set the info property.
        char gameinfo_prop[256];
        snprintf(gameinfo_prop, sizeof(gameinfo_prop), "%s %d %d", gamestart, game_pid, uidof(game_pid));
        systemv("/vendor/bin/setprop sys.azenith.gameinfo \"%s\"", gameinfo_prop);

        if (transsion_gamespace_support) {
            pid_t pid = fork();

            if (pid < 0) {
                // Fork failed, fall back to basic behavior
                log_zenith(LOG_ERROR, "Failed to fork for game session monitoring.");
                apply_profile(1);
                return;
            }

            if (pid > 0) {
                // This is the parent process.
                // Log the child PID and return immediately to not block the caller.
                log_zenith(LOG_INFO, "Forked child process %d to monitor game session.", pid);
                return;
            }

            // This is the child process. It will now run the monitoring loop.
            log_zenith(LOG_INFO, "Child process started for real-time sync.");
            sync_game_profile_loop(); // This function blocks until the game exits foreground.
            _exit(0); // IMPORTANT: Use _exit() to terminate the child process.

        } else {
            // Standard behavior without Transsion sync: apply performance profile and exit.
            log_zenith(LOG_INFO, "Game detected. Applying default performance profile.");
            apply_profile(1);
        }
    } else {
        // A non-game profile is requested (e.g., normal, powersave)
        systemv("/vendor/bin/setprop sys.azenith.gameinfo \"NULL 0 0\"");
        apply_profile(profile);
    }
}


char* get_gamestart(void) {
    return execute_command("/system/bin/dumpsys window visible-apps | /vendor/bin/grep 'package=.* ' | /vendor/bin/grep -Eo -f %s",
                           GAMELIST);
}

bool get_screenstate_normal(void) {
    static char fetch_failed = 0;
    char* screenstate = execute_command("/system/bin/dumpsys power | /vendor/bin/grep -Eo 'mWakefulness=Awake|mWakefulness=Asleep' | /system/bin/awk -F'=' '{print $2}'");
    if (screenstate) {
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

bool get_low_power_state_normal(void) {
    static char fetch_failed = 0;
    char* low_power = execute_direct("/system/bin/settings", "settings", "get", "global", "low_power", NULL);
    if (!low_power) {
        low_power = execute_command("/system/bin/dumpsys power | /vendor/bin/grep -Eo 'mSettingBatterySaverEnabled=true|mSettingBatterySaverEnabled=false' | /system/bin/awk -F'=' '{print $2}'");
    }
    if (low_power) {
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
