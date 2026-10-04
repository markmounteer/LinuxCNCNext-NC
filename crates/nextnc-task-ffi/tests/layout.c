/* Compile as both C11 and C++17 against the exact host header. No controller
 * is started; these assertions only qualify this compiler's wire layout. */
#include "nextnc_task.h"
#include <stddef.h>
#include <stdio.h>
#ifdef __cplusplus
#define CHECK static_assert
#else
#define CHECK _Static_assert
#endif
CHECK(NEXTNC_TASK_ABI == 5, "ABI version");
CHECK(sizeof(nextnc_spindle_evidence) == 80, "spindle evidence extent");
CHECK(offsetof(nextnc_spindle_evidence, identity) == 8, "spindle identity");
CHECK(offsetof(nextnc_spindle_evidence, maximum_rps) == 40, "spindle policy");
CHECK(sizeof(nextnc_timing_evidence) == 40, "timing evidence extent");
CHECK(offsetof(nextnc_timing_evidence, motion_birth) == 24, "timing birth");
CHECK(sizeof(nextnc_snapshot) == 1648, "snapshot extent");
CHECK(offsetof(nextnc_snapshot, trajectory) == 1216, "trajectory dynamics");
CHECK(offsetof(nextnc_snapshot, scalar_origin_mm) == 1240, "scalar origin");
CHECK(offsetof(nextnc_snapshot, spindle) == 1096, "spindle evidence");
CHECK(offsetof(nextnc_snapshot, timing) == 1176, "timing evidence");
CHECK(sizeof(nextnc_message) == 312, "message extent");
CHECK(offsetof(nextnc_message, feed_mm_s) == 272, "G94 feed");
CHECK(offsetof(nextnc_message, feed_mm_rev) == 280, "G95 feed");
CHECK(offsetof(nextnc_message, css_factor_rpm_mm) == 288, "CSS factor");
CHECK(offsetof(nextnc_message, css_maximum_rpm) == 296, "CSS cap");
CHECK(offsetof(nextnc_message, css_x_offset_mm) == 304, "CSS radius offset");
CHECK(sizeof(nextnc_motion_receipt) == 72, "receipt extent");
CHECK(offsetof(nextnc_motion_receipt, feed_per_rev) == 48, "recovery feed mode");
CHECK(offsetof(nextnc_motion_receipt, feed_mm_s) == 56, "recovery G94 feed");
CHECK(offsetof(nextnc_motion_receipt, feed_mm_rev) == 64, "recovery G95 feed");
int main(void) {
    printf("{\"abi\":%u,\"snapshot\":%zu,\"message\":%zu,\"receipt\":%zu}\n",
        NEXTNC_TASK_ABI, sizeof(nextnc_snapshot),sizeof(nextnc_message),sizeof(nextnc_motion_receipt));
    return 0;
}

CHECK(sizeof(nextnc_kernel_evidence) == 400, "kernel extent");
CHECK(offsetof(nextnc_snapshot, kernel) == 1248, "kernel offset");
