#ifndef NEXTNC_TASK_ABI_H
#define NEXTNC_TASK_ABI_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
#define NEXTNC_TASK_ABI 1u
#define NEXTNC_MAX_TOOLS 4096u
/* All dimensional values are mm, mm/s, mm/s^2, mm/s^3; X is lathe radius.
 * Native candidates are not execution permits. Preparation belongs on a worker.
 * Every pointer must remain valid for its declared extent; no input/output alias.
 * Functions return 0 on success, -1 on refusal, -2 on contained Rust panic. */
typedef struct {
    uint32_t abi, bytes, machine, axis_mask, work_offset, shaping, capabilities, reserved;
    double pose[9], work[9][9], rotation[9], temporary[9], tool_offset[9];
    double minimum[3], maximum[3], velocity[3], acceleration[3], jerk[3], maximum_rpm;
} nextnc_snapshot;
typedef struct { uint32_t number, reserved; double offset[9]; } nextnc_tool;
enum nextnc_kind {
    NEXTNC_LINE=1, NEXTNC_CIRCLE=2, NEXTNC_STATIONARY=3, NEXTNC_TERMINATION=4,
    NEXTNC_RESET_MODES=10, NEXTNC_CLEAR_TEMPORARY=11, NEXTNC_RESET_SPINDLE=12,
    NEXTNC_WORK_OFFSET=13, NEXTNC_TOOL_OFFSET=14, NEXTNC_FEED_PER_MINUTE=15,
    NEXTNC_CHANGE_TOOL=16, NEXTNC_SPINDLE=17, NEXTNC_COOLANT=18,
    NEXTNC_DWELL=19, NEXTNC_FENCE=20, NEXTNC_END=21
};
#define NEXTNC_RAPID 1u
#define NEXTNC_AT_SPEED 2u
#define NEXTNC_DRAIN_BEFORE 4u
#define NEXTNC_LAST_PIECE 8u
typedef struct {
    uint32_t abi, bytes, kind, flags;
    uint64_t command;
    uint32_t piece, pieces;
    int32_t argument, turn;
    double start[9], end[9], center[3], normal[3];
    double velocity, maximum_velocity, acceleration, jerk, value, feed_mm_s;
} nextnc_message;
uint32_t nextnc_task_abi(void);
int32_t nextnc_task_prepare(const uint8_t *, uint64_t, const nextnc_snapshot *, const nextnc_tool *, uint64_t, uint64_t *);
int32_t nextnc_task_commands(uint64_t, uint64_t *);
int32_t nextnc_task_piece(uint64_t, uint64_t, uint32_t, nextnc_message *, uint64_t);
int32_t nextnc_task_release(uint64_t);
uint64_t nextnc_task_error(uint8_t *, uint64_t);
#ifdef __cplusplus
}
#endif
#endif
