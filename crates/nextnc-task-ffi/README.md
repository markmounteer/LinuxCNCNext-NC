# Checked native task ABI

This is the small unsafe boundary around the safe Rust compiler, coordinate
binder and task-message lowering. ABI revision 3 exposes immutable prepared
candidates to C/C++. **A candidate handle is not permission to execute.** The
task-owner API now joins this candidate to the safe lifecycle and dispatch
ledger. Live snapshot acquisition, state reconciliation and actual queue dispatch
still require the pinned LinuxCNC host integration.

Revision 2 adds [native spindle feed and CSS](../../docs/native-spindle-protocol.md).
Its Rust protocol tests pass; the matching controller host and full native
G95/CSS simulator qualification are still pending. Revision-1 hosts and bodies
must not be mixed with this interface. No installed controller capability is
claimed by this development checkpoint.

Revision 3 adds [observed motion timing](../../../docs/native-timing-protocol.md).
The 1,216-byte snapshot requires rate-one cubic evidence and a motion instance
and birth identity. The matching controller provides this from actual motion
state; there is no implicit 1 ms default. Revisions 1 and 2 are refused before
reading their shorter bodies. Start, resume, tool confirmation and suffix rebind
must retain the prepared timing identity even when tool-table changes are allowed.

`include/nextnc_task.h` defines fixed-width, versioned structures. Inputs use
canonical millimetres, radius X and explicitly dimensioned dynamics. Machine,
axes, optional capability bits, field sizes, enum values, tool-table duplicates,
nonfinite data and resource bounds are checked before publishing any candidate.
The complete bundle is independently audited, bound and lowered before a handle
is returned. Heavy preparation must run off the task's cyclic control path.

At most two candidates/preparations may be outstanding. Handles are monotonic
process-local IDs and are never reused. Release of an attached candidate refuses;
detach first and release on the preparation worker, so a large job is not freed
on the cyclic task thread. Release revokes the handle; stale reads
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

The owner API is confined to its creating task thread, with one owner per process.
Begin a selection before file work, attach only the matching completed worker
generation, and start only after a fresh full snapshot fingerprint matches the
candidate. The fingerprint covers all nine pose components, every work/tool/G92
offset, controller dynamics and supported capabilities; it is independent of
tool-table iteration order and C struct padding. The host must separately verify
the selected source/policy identity and actual readiness.

Before RUN or confirmed STEP, the worker calls `nextnc_task_check_current` in
mode 0: selected bundle bytes and the full initial snapshot must still match.
Before held RESUME/confirmed STEP, mode 1 checks the same source and the bound
environment (work/rotation tables, tool table, limits, dynamics, capabilities and
shaper mode). Active WCS selection, G92, H and pose evolve through native events;
the host checks their live canonical consistency rather than comparing them to
the initial values. Mode 2 checks a tool confirmation's source and valid snapshot;
its mandatory result/rebind barrier still validates changed tool geometry before
further motion. A successful check returns a full observed fingerprint, never
execution permission or a new candidate. Immediately before adoption, the task
must match that fingerprint, selection, candidate and control request again.
Stops and replacement requests cancel pending verification before polling worker
output. The source check observes the bytes read by that worker; it is not a
continuous filesystem watch after execution starts.

`nextnc_owner_next` atomically reserves a complete source-command expansion and
returns a repeatable message reservation. The host checks its ordinary task
prerequisites before `issue`; only then may it call the guarded recipient, once.
`result` records that actual outcome. Unknown/refused delivery revokes admission.
All issue/result errors require host stop handling; a repeated reservation or
receipt never authorizes sending uncertain motion again.

Hold stops new issue before the host pauses motion. Resume-to-step requires the
effective semantic boundary returned by the owner. Completion requires all host
drain domains and a heartbeat after the last issue. MDI stays blocked until state
reconciliation is explicitly acknowledged. Abort/fault/disconnect revoke first,
even if their heartbeat is stale. These controls do not themselves move or stop
hardware: the host must perform the corresponding checked LinuxCNC operations.
After a tool command is receipted, `nextnc_owner_procedure` identifies the required
physical result. No suffix is offered until the task proves the exact prefix has
drained, checks the actual tool, and adopts `nextnc_task_rebind` worker output.
The worker re-reads the selected bundle, preserves the executed prefix and binds
every remaining command against a fresh snapshot. It rechecks travel, geometry,
shaper compatibility, independent T/H offsets and dynamics. The task rechecks the
full live fingerprint before adoption; changed state or stale worker identity is
refused. Adoption advances the state epoch and invalidates old receipts while
retaining command indexes and ledger serials. It never releases an existing hold.
A suffix candidate cannot become a new initial selection. Candidate retirement
still belongs on the worker after the owner detaches it.

Full host execution/reconciliation qualification remains unfinished Stage 3
work. The companion task now runs native
geometry/control simulations through actual recipients. Mode reset expands into
separately receipted termination and feed-mode changes before its task-state
update. Candidate attachment checks the maximum expansion against dispatch
capacity, using a worker-computed bound; no whole-job scan runs at attachment.

Stationary rapid records select G0 in both the immutable mode projection and the
companion task, preserving the previous feed and plane without inventing a motion
receipt. The companion motion-birth checkpoint at controller `31483c0f6f` and
compiler `1baf531dc1` passes 14 isolated simulator sessions, including repeated
jobs after graceful drained task replacement. Controller-owned stop after task
or connection loss and crashes during motion remain unqualified.
