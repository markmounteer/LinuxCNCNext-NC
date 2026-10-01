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
/* Worker-only rebind. Re-read the selected bundle; bytes must still match.
 * Previous candidate stays attached until owner_rebind adopts the result. */
int32_t nextnc_task_rebind(uint64_t previous, uint64_t completed, const uint8_t *, uint64_t,
    const nextnc_snapshot *, const nextnc_tool *, uint64_t, uint64_t *);
int32_t nextnc_task_commands(uint64_t, uint64_t *);
int32_t nextnc_task_piece(uint64_t, uint64_t, uint32_t, nextnc_message *, uint64_t);
int32_t nextnc_task_release(uint64_t);
uint64_t nextnc_task_error(uint8_t *, uint64_t);

/* Task lifecycle ABI. Preparation may run on a worker. Every owner call must
 * run on the thread which created the owner; there is one owner per process.
 * Fingerprints must be calculated from fresh host observations, not a bundle.
 * Attached candidates cannot be released: detach, then release on the worker. */
typedef struct {
    uint32_t abi, bytes;
    uint8_t configuration[32], initial[32];
} nextnc_fingerprint;
/* Worker-only: mode 0 initial RUN/STEP, 1 held RESUME/confirmed STEP,
 * 2 tool confirmation (mandatory suffix rebind still follows tool result).
 * Re-read bytes from the selected file. No execution permission is granted. */
int32_t nextnc_task_check_current(uint64_t candidate, uint32_t mode,
    const uint8_t *, uint64_t, const nextnc_snapshot *, const nextnc_tool *, uint64_t,
    nextnc_fingerprint *, uint64_t);
typedef struct {
    uint32_t abi, bytes;
    uint64_t selection, serial;
    uint32_t recipient, reserved; /* task=1, I/O=2, guarded motion=3 */
    nextnc_message message;
} nextnc_dispatch;
typedef struct {
    uint32_t abi, bytes, phase, allows_mdi;
    uint64_t selection, candidate, accepted, admitted, completed;
    uint64_t queued_pieces, accepted_pieces, pending_pieces, proposed_step_end;
} nextnc_owner_status_value;
typedef struct {
    uint32_t abi, bytes;
    uint64_t selection, serial, candidate, completed, state_epoch;
    uint32_t tool, reserved;
} nextnc_procedure;
/* Serial zero means no pending procedure. A nonzero result blocks all suffix
 * dispatch until the real tool result, physical drain and fresh binding agree. */
int32_t nextnc_owner_procedure(uint64_t, nextnc_procedure *, uint64_t);
int32_t nextnc_owner_rebind(uint64_t owner, uint64_t selection, uint64_t serial, uint64_t candidate,
    const nextnc_fingerprint *, uint32_t actual_tool, uint32_t drain_flags, uint64_t tick);
enum nextnc_owner_phase {
    NEXTNC_EMPTY=0, NEXTNC_LOADING=1, NEXTNC_SELECTED=2, NEXTNC_ARMED=3,
    NEXTNC_RUNNING=4, NEXTNC_HOLDING=5, NEXTNC_HELD=6, NEXTNC_STEP_DRAIN=7,
    NEXTNC_DRAINING=8, NEXTNC_RECONCILING=9, NEXTNC_COMPLETE=10,
    NEXTNC_ABORTING=11, NEXTNC_FAULTED=12
};
/* Fresh readiness bits, all seven required for initial start. Resume does not
 * require QUIESCENT: motion may be feed-held before its destination. */
#define NEXTNC_READY_AUTO 1u
#define NEXTNC_READY_ENABLED 2u
#define NEXTNC_READY_HOMED 4u
#define NEXTNC_READY_FAULT_FREE 8u
#define NEXTNC_READY_QUIESCENT 16u
#define NEXTNC_READY_BINDING_CURRENT 32u
#define NEXTNC_READY_DOWNSTREAM 64u
/* Physical completion bits. The runtime additionally enforces an observation
 * heartbeat strictly after the last dispatch or stop request. */
#define NEXTNC_DRAIN_TASK 1u
#define NEXTNC_DRAIN_IO 2u
#define NEXTNC_DRAIN_MOTION 4u
#define NEXTNC_DRAIN_IN_POSITION 8u
#define NEXTNC_DRAIN_SHAPER 16u
#define NEXTNC_DRAIN_FAULT_FREE 32u
enum nextnc_owner_operation {
    NEXTNC_HOLD=1,       /* argument=0, flags=0; close new issue before host hold */
    NEXTNC_HELD_ACK=2,   /* argument=host hold acknowledged (0/1), flags=0 */
    NEXTNC_RESUME=3,     /* argument=0 continuous or acknowledged step end; readiness flags */
    NEXTNC_PROPOSE_STEP=4, /* argument=0, flags=0; read proposed_step_end in status */
    NEXTNC_DRAINED=5,    /* argument=0; drain flags from actual host observations */
    NEXTNC_ABORT=6,      /* revoke BEFORE host cleanup; even stale tick stops */
    NEXTNC_FAULT=7,
    NEXTNC_DISCONNECTED=8,
    NEXTNC_RECONCILED=9  /* argument=state agrees (0/1); drain flags */
};
int32_t nextnc_task_fingerprint(const nextnc_snapshot *, const nextnc_tool *, uint64_t, nextnc_fingerprint *, uint64_t);
int32_t nextnc_owner_create(uint32_t capacity, uint64_t *);
int32_t nextnc_owner_destroy(uint64_t);
int32_t nextnc_owner_begin(uint64_t owner, uint64_t *selection);
int32_t nextnc_owner_attach(uint64_t owner, uint64_t selection, uint64_t candidate);
int32_t nextnc_owner_failed(uint64_t owner, uint64_t selection);
int32_t nextnc_owner_start(uint64_t owner, const nextnc_fingerprint *, uint64_t state_epoch, uint32_t readiness, uint32_t mode, uint64_t restart);
/* next() reserves the whole source-command expansion. serial=0 means nothing
 * offered. Repeated next() is inert; issue() authorizes exactly one recipient
 * call and cannot be retried. result outcomes: 0 accepted, 1 rejected, 2 unknown.
 * Accepted is NOT completed. Unknown/rejected closes admission; host MUST stop.
 * Host must also stop on an issue/result ABI error or a contained panic. */
int32_t nextnc_owner_next(uint64_t, nextnc_dispatch *, uint64_t);
int32_t nextnc_owner_issue(uint64_t, uint64_t selection, uint64_t serial, uint64_t tick);
int32_t nextnc_owner_result(uint64_t, uint64_t selection, uint64_t serial, uint32_t outcome, uint64_t tick);
/* Immutable expected modes from the successfully accepted motion-piece prefix.
 * This is NOT evidence of execution or permission to release task ownership.
 * motion: 1 rapid, 2 line, 3 clockwise arc, 4 counterclockwise arc.
 * plane: 0 inherit launch plane, 1 XY, 2 XZ, 3 YZ.
 * feed_known=0 inherits launch feed; otherwise feed_mm_s is authoritative. */
typedef struct {
    uint32_t abi, bytes;
    uint64_t selection, serial, command;
    uint32_t piece, motion, plane, feed_known;
    double feed_mm_s;
} nextnc_motion_receipt;
int32_t nextnc_owner_motion_receipt(uint64_t, uint64_t selection, uint64_t serial,
    nextnc_motion_receipt *, uint64_t);
int32_t nextnc_owner_control(uint64_t, uint32_t operation, uint64_t argument, uint32_t flags, uint64_t tick);
int32_t nextnc_owner_status(uint64_t, nextnc_owner_status_value *, uint64_t);
#ifdef __cplusplus
}
#endif
#endif
