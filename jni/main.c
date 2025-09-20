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
unsigned int LOOP_INTERVAL = 5;
char* gamestart = NULL;
bool preload_active = false;
bool did_log_preload = true;
pid_t game_pid = 0;

int main(void) {


    // Register signal handlers
    signal(SIGINT, sighandler);
    signal(SIGTERM, sighandler);


    // Initialize variables
    bool need_profile_checkup = false;
    MLBBState mlbb_is_running = MLBB_NOT_RUNNING;
    ProfileMode cur_mode = PERFCOMMON;
    static bool did_notify_start = false;

    log_zenith(LOG_INFO, "Daemon started as PID %d", getpid());
    cleanup_vmt();
    run_profiler(PERFCOMMON);

    while (1) {
        sleep(LOOP_INTERVAL);

        // Apply frequencies
        if (get_screenstate()) {
            if (cur_mode == BALANCED_PROFILE)
                systemv("AZenith_Profiler setsfreqs");
            else if (cur_mode == ECO_MODE)
                systemv("AZenith_Profiler setsfreqs");
            else if (cur_mode == PERFORMANCE_PROFILE)
                systemv("AZenith_Profiler apply_game_freqs");
        } else {
            // Screen Off, Do Nothing
        }

        // Only fetch gamestart when user not in-game
        // prevent overhead from dumpsys commands.
        if (!gamestart) {
            gamestart = get_gamestart();
        } else if (game_pid != 0 && kill(game_pid, 0) == -1) [[clang::unlikely]] {
            log_zenith(LOG_INFO, "Game %s exited, resetting profile...", gamestart);
            stop_preloading(&LOOP_INTERVAL);
            game_pid = 0;
            free(gamestart);
            gamestart = get_gamestart();

            // Force profile recheck to make sure new game session get boosted
            need_profile_checkup = true;
        }

        if (gamestart)
            mlbb_is_running = handle_mlbb(gamestart);

        if (gamestart && get_screenstate() && mlbb_is_running != MLBB_RUN_BG) {
            // Preload assets for the game
            preload(gamestart, &LOOP_INTERVAL);
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
            if (!did_log_preload) {
                log_zenith(LOG_INFO, "Start Preloading game package %s", gamestart);
                notify("Start Preloading game package");
                did_log_preload = true;
            }
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
            if (!did_notify_start) {
                notify("AZenith is running successfully");
                did_notify_start = true;
            }
            run_profiler(BALANCED_PROFILE);
        }
    }

    return 0;
}
