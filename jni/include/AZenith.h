#ifndef AZENITH_H
#define AZENITH_H

#include <ctype.h>
#include <dirent.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#define MAX_DATA_LENGTH 1024
#define MAX_COMMAND_LENGTH 600
#define MAX_OUTPUT_LENGTH 256
#define MAX_PATH_LENGTH 256

#define NOTIFY_TITLE "AZenith"
#define LOG_TAG "AZenith"

#define GAMELIST "/sdcard/gamelist.txt"
#define LOOP_INTERVAL 15
#define MY_PATH                                                                      \
    "PATH=/vendor/bin/hw"

#define IS_MLBB(gamestart)                                                                               \
    (strcmp(gamestart, "com.mobile.legends") == 0 || strcmp(gamestart, "com.mobilelegends.hwag") == 0 || \
     strcmp(gamestart, "com.mobiin.gp") == 0 || strcmp(gamestart, "com.mobilechess.gp") == 0)

#define IS_AWAKE(state) (strcmp(state, "Awake") == 0 || strcmp(state, "true") == 0)
#define IS_LOW_POWER(state) (strcmp(state, "true") == 0 || strcmp(state, "1") == 0)

// Basic C knowledge: enum starts with 0

typedef enum : char {
    LOG_DEBUG,
    LOG_INFO,
    LOG_WARN,
    LOG_ERROR,
    LOG_FATAL
} LogLevel;

typedef enum : char {
    PERFCOMMON,
    PERFORMANCE_PROFILE,
    BALANCED_PROFILE,
    ECO_MODE
} ProfileMode;

typedef enum : char {
    MLBB_NOT_RUNNING,
    MLBB_RUN_BG,
    MLBB_RUNNING
} MLBBState;

extern char* gamestart;
extern char* custom_log_tag;
extern pid_t game_pid;

/*
 * If you're here for function comments, you
 * are in the wrong place.
 */

static bool preload_active = false;
void sighandler(const int signal);
char* trim_newline(char* string);
char* timern(void);
bool return_true(void);
bool return_false(void);

// Shell and Command execution
char* execute_command(const char* format, ...);
char* execute_direct(const char* path, const char* arg0, ...);
int systemv(const char* format, ...);

// Utilities
int write2file(const char* filename, const bool append, const bool use_flock, const char* data, ...);

// system
void log_preload(LogLevel level, const char* message, ...);
void log_zenith(LogLevel level, const char* message, ...);

// Utilities
void set_priority(const pid_t pid);
pid_t pidof(const char* name);
int uidof(pid_t pid);

// Handler
extern pid_t mlbb_pid;
MLBBState handle_mlbb(const char* gamestart);

// Profiler
extern bool (*get_screenstate)(void);
extern bool (*get_low_power_state)(void);
void setup_path(void);
char* get_gamestart(void);
bool get_screenstate_normal(void);
bool get_low_power_state_normal(void);
void run_profiler(const int profile);

#endif
