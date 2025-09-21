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
#include <stdlib.h> // For general utilities
#include <sys/system_properties.h> // For native property access

// Define binary paths for easier maintenance
#define DUMPSYS_PATH  "/system/bin/dumpsys"
#define PROFILER_PATH "/vendor/bin/AZenith_Profiler"
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

// Use the modern NDK functions to support keys longer than 31 chars.
static char* get_prop(const char* prop) {
    const prop_info* pi = __system_property_find(prop);
    if (pi == NULL) {
        return NULL; // Property does not exist
    }

    char value[PROP_VALUE_MAX];
    if (__system_property_read(pi, NULL, value) > 0) {
        return strdup(value);
    }

    return strdup(""); // Return empty string if property exists but has no value
}


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
        char* support_prop = get_prop("persist.sys.azenith.syncgamespace.support");
        char* is_transsion_prop = get_prop("persist.sys.azenith.issupportgamespace");

        if (support_prop && is_transsion_prop &&
            strcmp(support_prop, "1") == 0 &&
            strcmp(is_transsion_prop, "transsion") == 0) {
            transsion_gamespace_support = true;
            log_zenith(LOG_INFO, "Transsion Game Space support detected and enabled.");
        }

        free(support_prop);
        free(is_transsion_prop);
        gamespace_props_checked = true;
    }

    // If game profile is requested and Transsion support=1 then sync with Transsion's game space setting.
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
    char profile_str[4]; // Buffer for integer to string conversion

    // system property ndk
    if (profile == 1) {
        snprintf(gameinfo_prop, sizeof(gameinfo_prop), "%s %d %d", gamestart, game_pid, uidof(game_pid));
        __system_property_set("sys.azenith.gameinfo", gameinfo_prop);
    } else {
        __system_property_set("sys.azenith.gameinfo", "NULL 0 0");
    }

    snprintf(profile_str, sizeof(profile_str), "%d", final_profile);
    __system_property_set("sys.azenith.currentprofile", profile_str);

    // Execute the profiler script
    systemv(PROFILER_PATH " %d", final_profile);
}

/***********************************************************************************
 * Helper Function    : read_gamelist
 * Description        : Reads package names from the GAMELIST file into a dynamically
 * allocated array of strings.
 * Returns            : The number of packages read. Caller is responsible for
 * freeing the allocated memory.
 ***********************************************************************************/
static int read_gamelist(char*** game_packages) {
    FILE* file = fopen(GAMELIST, "r");
    if (!file) {
        log_zenith(LOG_ERROR, "Could not open gamelist file: %s", GAMELIST);
        return 0;
    }

    *game_packages = NULL;
    int count = 0;
    char line[256];

    while (fgets(line, sizeof(line), file)) {
        line[strcspn(line, "\r\n")] = 0; // Remove trailing newline
        if (strlen(line) == 0) continue;

        *game_packages = realloc(*game_packages, (count + 1) * sizeof(char*));
        if (!*game_packages) {
             log_zenith(LOG_FATAL, "Memory allocation failed for gamelist");
             for (int i=0; i < count; i++) free((*game_packages)[i]);
             free(*game_packages);
             fclose(file);
             return 0;
        }
        (*game_packages)[count] = strdup(line);
        count++;
    }

    fclose(file);
    return count;
}


/***********************************************************************************
 * Function Name      : get_gamestart
 * Description        : Searches for a visible app that matches a package in the gamelist.
 * This version parses dumpsys output directly in C to avoid
 * spawning grep processes.
 * Note               : Caller is responsible for freeing the returned string.
 ***********************************************************************************/
char* get_gamestart(void) {
    char** game_packages = NULL;
    int game_count = read_gamelist(&game_packages);
    if (game_count == 0) return NULL;

    char* visible_apps = execute_command(DUMPSYS_PATH " window visible-apps");
    char* found_game_package = NULL;

    if (visible_apps) {
        char* current_pos = visible_apps;
        char* next_line;
        
        while ((next_line = strchr(current_pos, '\n')) != NULL) {
            *next_line = '\0';
            
            for (int i = 0; i < game_count; i++) {
                if (strstr(current_pos, "package=") && strstr(current_pos, game_packages[i])) {
                    found_game_package = strdup(game_packages[i]);
                    goto cleanup;
                }
            }
            current_pos = next_line + 1;
        }
        
        if (*current_pos != '\0') {
             for (int i = 0; i < game_count; i++) {
                if (strstr(current_pos, "package=") && strstr(current_pos, game_packages[i])) {
                    found_game_package = strdup(game_packages[i]);
                    goto cleanup;
                }
            }
        }
    }

cleanup:
    free(visible_apps);
    for (int i = 0; i < game_count; i++) free(game_packages[i]);
    free(game_packages);

    return found_game_package;
}

/***********************************************************************************
 * Function Name      : get_screenstate_normal
 * Description        : Retrieves screen wakefulness by parsing dumpsys output directly
 * in C, avoiding grep and awk.
 ***********************************************************************************/
bool get_screenstate_normal(void) {
    static char fetch_failed = 0;
    char* power_dump = execute_command(DUMPSYS_PATH " power");

    if (power_dump) {
        char* wakefulness = strstr(power_dump, "mWakefulness=");
        if (wakefulness) {
            bool is_awake = (strstr(wakefulness, "Awake") != NULL);
            free(power_dump);
            fetch_failed = 0;
            return is_awake;
        }
        // If parsing fails, log the output for debugging before freeing.
        log_zenith(LOG_DEBUG, "get_screenstate: Unexpected dumpsys output: %s", power_dump);
        free(power_dump);
    }

    fetch_failed++;
    log_zenith(LOG_ERROR, "Unable to fetch current screenstate");

    if (fetch_failed >= 6) {
        log_zenith(LOG_FATAL, "get_screenstate is out of order!");
        get_screenstate = return_true;
    }
    return true;
}

/***********************************************************************************
 * Function Name      : get_low_power_state_normal
 * Description        : Checks Battery Saver status. The fallback method now parses
 * dumpsys output in C instead of using grep/awk.
 ***********************************************************************************/
bool get_low_power_state_normal(void) {
    static char fetch_failed = 0;
    char* low_power = execute_direct(SETTINGS_PATH, "settings", "get", "global", "low_power", NULL);

    if (!low_power || strcmp(low_power, "null") == 0 || strlen(low_power) == 0) {
        if (low_power) free(low_power);

        char* power_dump = execute_command(DUMPSYS_PATH " power");
        if (power_dump) {
            char* setting = strstr(power_dump, "mSettingBatterySaverEnabled=");
            if (setting) {
                if (strstr(setting, "true")) {
                    low_power = strdup("true");
                } else {
                    low_power = strdup("false");
                }
            } else {
                // If parsing fails, log the output for debugging before freeing.
                log_zenith(LOG_DEBUG, "get_low_power_state: Unexpected dumpsys output: %s", power_dump);
                low_power = NULL;
            }
            free(power_dump);
        } else {
            low_power = NULL;
        }
    }

    if (low_power) [[clang::likely]] {
        fetch_failed = 0;
        bool result = IS_LOW_POWER(low_power);
        free(low_power);
        return result;
    }

    fetch_failed++;
    log_zenith(LOG_ERROR, "Unable to fetch battery saver status");

    if (fetch_failed >= 6) {
        log_zenith(LOG_FATAL, "get_low_power_state is out of order!");
        get_low_power_state = return_false;
    }

    return false;
}

