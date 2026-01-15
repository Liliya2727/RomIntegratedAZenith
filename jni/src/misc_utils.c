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
#include <sys/system_properties.h>

/***********************************************************************************
 * Function Name      : trim_newline
 * Inputs             : str (char *) - string to trim newline from
 * Returns            : char * - string without newline
 * Description        : Trims a newline character at the end of a string if
 * present.
 ***********************************************************************************/
[[gnu::always_inline]] char* trim_newline(char* string) {
    if (string == NULL)
        return NULL;

    char* end;
    if ((end = strchr(string, '\n')) != NULL)
        *end = '\0';

    return string;
}

/***********************************************************************************
 * Function Name      : notify
 * Inputs             : message (char *) - Message to display
 * Returns            : None
 * Description        : Push a notification.
 ***********************************************************************************/
void notify(const char* message) {
    int exit = systemv("su -lp 2000 -c \"/system/bin/cmd notification post "
                       "-t '%s' "
                       "'AZenith' '%s'\" >/dev/null",
                       NOTIFY_TITLE, message);

    if (exit != 0) [[clang::unlikely]] {
        log_zenith(LOG_ERROR, "Unable to post push notification, message: %s", message);
    }
}

/***********************************************************************************
 * Function Name      : timern
 * Inputs             : None
 * Returns            : char * - pointer to a statically allocated string
 * with the formatted time.
 * Description        : Generates a timestamp with the format
 * [YYYY-MM-DD HH:MM:SS.milliseconds].
 ***********************************************************************************/
char* timern(void) {
    static char timestamp[64];
    struct timeval tv;
    time_t current_time;
    struct tm* local_time;

    gettimeofday(&tv, NULL);
    current_time = tv.tv_sec;
    local_time = localtime(&current_time);

    if (local_time == NULL) [[clang::unlikely]] {
        strcpy(timestamp, "[TimeError]");
        return timestamp;
    }

    size_t format_result = strftime(timestamp, sizeof(timestamp), "%Y-%m-%d %H:%M:%S", local_time);
    if (format_result == 0) [[clang::unlikely]] {
        strcpy(timestamp, "[TimeFormatError]");
        return timestamp;
    }

    // Append milliseconds
    snprintf(timestamp + strlen(timestamp), sizeof(timestamp) - strlen(timestamp), ".%03ld", tv.tv_usec / 1000);

    return timestamp;
}

/***********************************************************************************
 * Function Name      : sighandler
 * Inputs             : int signal - exit signal
 * Returns            : None
 * Description        : Handle exit signal.
 ***********************************************************************************/
[[noreturn]] void sighandler(const int signal) {
    switch (signal) {
    case SIGTERM:
        log_zenith(LOG_INFO, "Received SIGTERM, exiting.");
        break;
    case SIGINT:
        log_zenith(LOG_INFO, "Received SIGINT, exiting.");
        break;
    }

    // Exit gracefully
    _exit(EXIT_SUCCESS);
}

/***********************************************************************************
 * Function Name      : return_true
 * Inputs             : None
 * Returns            : bool - only true
 * Description        : Will be used for error fallback.
 * Note               : Never call this function.
 ***********************************************************************************/
bool return_true(void) {
    return true;
}

/***********************************************************************************
 * Function Name      : return_false
 * Inputs             : None
 * Returns            : bool - only false
 * Description        : Will be used for error fallback.
 * Note               : Never call this function.
 ***********************************************************************************/
bool return_false(void) {
    return false;
}
/***********************************************************************************
 * Function Name      : cleanup
 * Inputs             : None
 * Returns            : None
 * Description        : kill preload process
 ***********************************************************************************/
void cleanup_vmt(void) {
    int pr1 = systemv("/system/bin/toybox pidof sys.azenith-preloadbin > /dev/null 2>&1");
    int pr2 = systemv("/system/bin/toybox pidof sys.azenith-preloadbin2 > /dev/null 2>&1");
    if (pr1 == 0 || pr2 == 0) {
        log_zenith(LOG_INFO, "Killing restover preload processes");
        systemv("pkill -9 -f sys.azenith-preloadbin");
        systemv("pkill -9 -f sys.azenith-preloadbin2");
    }
}

/***********************************************************************************
 * Function Name      : preload
 * Inputs             : gamepkg
 * Returns            : None
 * Description        : Run preloads on loop
 ***********************************************************************************/
void preload(const char* pkg, unsigned int* LOOP_INTERVAL) {
    char val[PROP_VALUE_MAX] = {0};
    if (__system_property_get("persist.sys.azenithconf.gpreload", val) > 0) {
        if (val[0] == '1') {
            pid_t pid = fork();
            if (pid == 0) {
                GamePreload(pkg);
                _exit(0);
            } else if (pid > 0) {
                *LOOP_INTERVAL = 35;
                did_log_preload = false;
                preload_active = true;
            } else {
                log_zenith(LOG_ERROR, "Failed to fork process for GamePreload");
            }
        }
    }
}

/***********************************************************************************
 * Function Name      : stop preload
 * Inputs             : none
 * Returns            : None
 * Description        : stop if preload is running
 ***********************************************************************************/
void stop_preloading(unsigned int* LOOP_INTERVAL) {
    if (preload_active) {
        cleanup_vmt();
        notify("Preload Stopped");
        *LOOP_INTERVAL = 15;
        did_log_preload = true;
        preload_active = false;
    }
}

/***********************************************************************************
 * Function Name      : get_gamelist_path
 * Inputs             : none
 * Returns            : char* path to gamelist
 * Description        : Checks prop persist.sys.azenith.gamelist for custom path otherwise returns default /sdcard/gamelist.txt
 ***********************************************************************************/
char* get_gamelist_path(void) {
    static char path[MAX_PATH_LENGTH];
    char prop_val[PROP_VALUE_MAX] = {0};

    // Check if the property exists and is not empty
    if (__system_property_get("persist.sys.azenith.gamelist", prop_val) > 0 && strlen(prop_val) > 0) {
        snprintf(path, sizeof(path), "%s", prop_val);
    } else {
        snprintf(path, sizeof(path), "/sdcard/gamelist.txt");
    }
    return path;
}
