# Checked native task preparation ABI

This is the small unsafe boundary around the safe Rust compiler, coordinate
binder and task-message lowering. ABI revision 1 exposes immutable prepared
candidates to C/C++. **A candidate handle is not permission to execute.** The
native task lifecycle, live snapshot acquisition, state reconciliation and actual
queue dispatch remain required integration work.

`include/nextnc_task.h` defines fixed-width, versioned structures. Inputs use
canonical millimetres, radius X and explicitly dimensioned dynamics. Machine,
axes, optional capability bits, field sizes, enum values, tool-table duplicates,
nonfinite data and resource bounds are checked before publishing any candidate.
The complete bundle is independently audited, bound and lowered before a handle
is returned. Heavy preparation must run off the task's cyclic control path.

At most two candidates/preparations may be outstanding. Handles are monotonic
process-local IDs and are never reused. Release revokes the handle; stale reads
fail. Read operations are immutable and do not issue, acknowledge or complete a
motion. Aborting an executing job must be owned by the separate task lifecycle,
not inferred from candidate release. Failed preparation clears its output handle;
it cannot select an earlier candidate automatically.

All C pointers must reference aligned, live storage of the declared extent and
must not alias input/output buffers. Length, alignment and null checks do not
make arbitrary addresses safe. The common snapshot header is checked before the
versioned body is read. Rust panics are contained at result-bearing entry points;
allocation failure that aborts the process is not claimed recoverable.

Return values are zero for success, -1 for refusal and -2 for a contained panic.
`nextnc_task_error` reads the calling thread's last diagnostic. Worker code must
capture that text before returning its result to the task. Diagnostics never
replace checking the return code.

The message `kind` identifies each payload; unused fields are zero. `argument`
holds a WCS/tool/H index, termination condition (0 stop, 1 exact, 2 blend), spindle
direction (0 stop, +1 clockwise, -1 counterclockwise), or coolant (0 off, 1 flood,
2 mist). `value` holds rotation degrees, RPM, dwell seconds or termination
tolerance as appropriate. `start`/`end` are full nine-component poses; offset
messages use `end`. A circular normal is a positive principal unit vector, with
direction/multiple revolutions represented by LinuxCNC's signed `turn` convention.

`NEXTNC_DRAIN_BEFORE`, `NEXTNC_AT_SPEED` and `NEXTNC_LAST_PIECE` are semantic
requirements. A host must enforce them; they are not informational display flags.
`NEXTNC_STATIONARY` preserves source identity and any entry gate without sending a
zero-length planner line. Motion and state pieces share the original prepared
command index and a zero-based piece ordinal.

The companion controller's `nextnc-stage3` container consumes this ABI through
actual pinned NML types. Its 52-case geometry/representation audit covers all
26 task fixtures in millimetre and inch external units. That audit does not start
LinuxCNC or prove native execution; the complete Stage 3 acceptance gate remains
open. Rust tests also exercise invalid ABI/state data, bounded capacity and stale
handles. No Node process is used by the library.
