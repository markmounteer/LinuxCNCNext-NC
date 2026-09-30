# Native task execution components — Stage 3 in progress

This safe Rust library implements the control and coordinate components of the
planned LinuxCNC task integration. **It does not execute a job.** The companion
`nextnc-task-ffi` crate provides checked preparation, owner control and dispatch
accounting. The experimental pinned task now loads/binds jobs, gates competing
commands and issues native records through existing task recipients. Initial
normal/abort state handoff is implemented and under simulator qualification. Native
full-stack acceptance is pending. The compiler CLI reports `executable:false`.

## Implemented components

- `lifecycle`: one AUTO owner, generation-bound selection/start, bounded offers,
  absolute acknowledged prefixes, partial receipt retries, hold/resume,
  acknowledged semantic stepping, drain/rebind/reconciliation and immediate
  admission revocation for abort/fault/disconnect. Failed selection never restores
  an old runnable job. MDI stays unavailable until explicit reconciliation.
- `steps`: versioned groups keep every source use and every expanded polyline
  segment. Analytic helices remain single groups; tools, dwells, fences and end
  require drains. Native restart-at-command indexes are rejected.
- `binding`: whole-job XYZ/XZ mm/radius binding against supplied live-state data,
  with work/G92/tool transforms, independent T/H lookup, partial-axis reviewed
  waypoints, analytic arcs/helixes and continuous path travel checks. Auxiliary
  positions/offsets are checked and unsupported nonzero values are refused.
  Supported principal-plane work rotations preserve analytic geometry.
- Shaper compatibility: shaped XY or pure-Z bypass; mixed XY/Z and incompatible
  circles/helixes are refused. Switching lanes inserts a drain requirement without
  adding synthetic motion or altering semantic step groups.
- `receipts`: bounded expanded-message accounting, one issue per ticket, actual
  task/I/O/guarded-motion results, command admission only after all pieces, and
  observation ticks newer than dispatch. Uncertain delivery closes admission;
  retrying a receipt cannot resend a motion. A fatal receipt retains source identity.
- `lowering`: whole-job task pieces with directional velocity/acceleration/jerk,
  analytic circular turn counts, explicit stationary-source receipts and ordered
  termination deltas. Policy changes add owner drain barriers. Scalar lowering
  does not replace the planner's own limits or qualify motion timing.

Tests supply synthetic host observations. They prove Rust state transitions and
geometry binding under those observations; they do **not** prove that LinuxCNC
provides them correctly. In particular, queued, dispatched, admitted and physically
completed are separate states. `Ledger::observed_after_dispatch` does not prove a
motion or shaper drain. The owning host must combine it with real status.

## Remaining integration gates

1. Complete procedure result/rebind transport and its fresh-state checks. The
   companion pinned C++ adapter now queues native records through the task and
   records actual recipient returns. Full failure/retry qualification is pending.
2. Complete live snapshot qualification and identity/freshness checks. The task
   captures state and a bounded worker binds the whole job. Recheck before start and after procedures;
   cached snapshots cannot authorize execution.
3. The native AUTO branch in the existing task. All command-channel and
   HALUI/pendant starts must use the same owner; hold/abort remain serviceable at
   capacity. Tool changes require a real procedure result, not a HAL loopback.
4. Verified canonical/interpreter state reconciliation at normal end and abort,
   including `on_abort` ordering. Do not restore a cached pose or issue generated
   G-code as a substitute for the typed backend.
5. Native full-stack LinuxCNC simulation through task, guarded motmod and the
   actual Rust shim: coordinate/offset matrix, event order, termination changes,
   retry/fault injection, delayed shaper tails and native-to-MDI handoff.

G95/spindle synchronization and CSS are explicit Stage 4 requirements and are
refused by this Stage 3 binder. GUI work and physical-machine commissioning are
outside this checkpoint.

## Reproduction

From the repository root, using its pinned Rust toolchain:

```text
cargo test --locked -p nextnc-task
cargo clippy --locked -p nextnc-task --all-targets -- -D warnings
```

The 26 fixtures under `tests/fixtures` were generated from the pinned Fusion
producer by `tools/native-task-fixtures/generate.cjs`. Their manifest records the
producer revision, generator hash, exact source/setup hashes and independent
expected geometry. The matrix test verifies the hashes. Node is used only to
regenerate development fixtures, not by the Rust library or native runtime.
