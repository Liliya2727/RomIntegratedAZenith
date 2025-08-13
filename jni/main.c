/*
 * Copyright (C) 2024-2025 Rem01Gaming
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
#include <libgen.h>

unsigned int LOOP_INTERVAL = 15;


char* gamestart = NULL;
pid_t game_pid = 0;

int main(int argc, char* argv[]) {

    // Expose logging interface for other modules
    char* base_name = basename(argv[0]);
    if (strcmp(base_name, "AZenith_log") == 0) {
        if (argc < 3) {
            fprintf(stderr, "Usage: AZenith_log <TAG> <LEVEL> <MESSAGE>\n");
            fprintf(stderr, "Levels: 0=DEBUG, 1=INFO, 2=WARN, 3=ERROR, 4=FATAL\n");
            return EXIT_FAILURE;
        }

        // Parse log level
        int level = atoi(argv[2]);
        if (level < LOG_DEBUG || level > LOG_FATAL) {
            fprintf(stderr, "Invalid log level. Use 0-4.\n");
            return EXIT_FAILURE;
        }

        // Combine message arguments
        size_t message_len = 0;
        for (int i = 3; i < argc; i++) {
            message_len += strlen(argv[i]) + 1;
        }

        char message[message_len];
        message[0] = '\0';

        for (int i = 3; i < argc; i++) {
            strcat(message, argv[i]);
            if (i < argc - 1)
                strcat(message, " ");
        }
        return EXIT_SUCCESS;
    }


    // Daemonize service
    if (daemon(0, 0)) {
        log_zenith(LOG_FATAL, "Unable to daemonize service");
        exit(EXIT_FAILURE);
    }

    // Register signal handlers
    signal(SIGINT, sighandler);
    signal(SIGTERM, sighandler);

    // Initialize variables
    bool need_profile_checkup = false;
    MLBBState mlbb_is_running = MLBB_NOT_RUNNING;
    ProfileMode cur_mode = PERFCOMMON;

    log_zenith(LOG_INFO, "Daemon started as PID %d", getpid());
    run_profiler(PERFCOMMON); // exec perfcommon

    while (1) {
        sleep(LOOP_INTERVAL);


        // Only fetch gamestart when user not in-game
        // prevent overhead from dumpsys commands.
        if (!gamestart) {
            gamestart = get_gamestart();
        } else if (game_pid != 0 && kill(game_pid, 0) == -1) [[clang::unlikely]] {
            log_zenith(LOG_INFO, "Game %s exited, resetting profile...", gamestart);
            game_pid = 0;
            free(gamestart);
            gamestart = get_gamestart();

            // Force profile recheck to make sure new game session get boosted
            need_profile_checkup = true;
        }

        if (gamestart)
            mlbb_is_running = handle_mlbb(gamestart);

        if (gamestart && get_screenstate() && mlbb_is_running != MLBB_RUN_BG) {
            // Bail out if we already on performance profile
            if (!need_profile_checkup && cur_mode == PERFORMANCE_PROFILE)
                continue;

            // Get PID and check if the game is "real" running program
            // Handle weird behavior of MLBB
            game_pid = (mlbb_is_running == MLBB_RUNNING) ? mlbb_pid : pidof(gamestart);
            if (game_pid == 0) [[clang::unlikely]] {
                log_zenith(LOG_ERROR, "Unable to fetch PID of %s", gamestart);
                free(gamestart);
                gamestart = NULL;
                continue;
            }

            cur_mode = PERFORMANCE_PROFILE;
            need_profile_checkup = false;
            log_zenith(LOG_INFO, "Applying performance profile for %s", gamestart);
            run_profiler(PERFORMANCE_PROFILE);
            set_priority(game_pid);
        } else if (get_low_power_state()) {
            // Bail out if we already on powersave profile
            if (cur_mode == ECO_MODE)
                continue;

            cur_mode = ECO_MODE;
            need_profile_checkup = false;
            log_zenith(LOG_INFO, "Applying ECO Mode");
            run_profiler(ECO_MODE);
        } else {
            // Bail out if we already on normal profile
            if (cur_mode == BALANCED_PROFILE)
                continue;

            cur_mode = BALANCED_PROFILE;
            need_profile_checkup = false;
            log_zenith(LOG_INFO, "Applying Balanced profile");
            run_profiler(BALANCED_PROFILE);
        }
    }

    return 0;
}
