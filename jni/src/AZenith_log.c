/*
 * Copyright (C) 2024-2025 Zexshia
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include <AZenith.h>
#include <android/log.h>

char* custom_log_tag = NULL;
const char* level_str[] = {"D", "I", "W", "E", "F"};

/***********************************************************************************
 * Function Name      : log_zenith
 * Inputs             : level - Log level
 *                      message (const char *) - message to log
 *                      variadic arguments - additional arguments for message
 * Returns            : None
 * Description        : print and logs a formatted message with a timestamp
 *                      to a log file.
 ***********************************************************************************/
void log_zenith(LogLevel level, const char* message, ...) {
    char logMesg[MAX_OUTPUT_LENGTH];
    va_list args;
    va_start(args, message);
    vsnprintf(logMesg, sizeof(logMesg), message, args);
    va_end(args);

    int android_log_level;
    switch (level) {
        case LOG_INFO: android_log_level = ANDROID_LOG_INFO; break;
        case LOG_WARN: android_log_level = ANDROID_LOG_WARN; break;
        case LOG_ERROR: android_log_level = ANDROID_LOG_ERROR; break;
        default: android_log_level = ANDROID_LOG_DEBUG; break;
    }

    __android_log_print(android_log_level, LOG_TAG, "%s", logMesg);
}
