# Stage 3 progress — native task execution is not accepted

Date: 2026-09-30. **IN PROGRESS. Native jobs now execute in the experimental
LinuxCNC task simulator; full Stage 3 acceptance remains incomplete.** The completed Stage 0 simulator reference and
[Stages 1–2](../stages12/README.md) remain distinct evidence.

The latest checkpoint binds native commands to an independent **motion birth
identity** at controller `31483c0f6fccb9288ca4c175048c816d34463e46` and compiler
`1baf531dc1b4f672983cb5ef2a14b09d567a3e24`. Fourteen sessions pass: eight execution
controls, two aborts after admission but before any pose output, two mismatched
motion-identity refusals and two graceful task-replacement/repeated-job sequences.
Twelve competing commands are refused after the identity faults. Restart testing
found roundoff-only positioning moves; this compiler now treats those reviewed
waypoints as stationary while retaining source cutting geometry and rapid modal
effects. The motion receipt check is unchanged. The controller's
`2026-09-30-motion-birth` evidence retains 536 raw files, including both failed
restart baselines and the preceding twelve passing probes, with independent
committed-byte verification and six rehashed semantic challenges. All 39 local
task/ABI tests and Clippy pass; the task/ABI suites also pass in Linux. The shim
binary matches the retained build with 290 passing tests and four ignored.

Controller-owned stopping on task/connection loss, task crash during motion,
full-stack braking-boundary crossing, recovery from latched uncertainty and the
remaining Stage 3 matrices stay open. No physical controller was used.

The preceding checkpoint adds **motion-owned execution provenance** at
runtime `436ae05724c530d2b54f4a26fe5272d8ed493c9a`, with this compiler/runtime
unchanged at `68fe04696876706e53b6496f0406d34b369892bb`. Fourteen simulator sessions
pass: eight completion/abort/step controls and six stopped-receipt faults across
the mill and lathe. Two baselines show whole accepted-receipt replay restoring
G1/F480 after actual G3/F120 motion. The Rust shim now publishes successful
admission and commanded-pose serials under a coherent HAL snapshot; the task
matches the stopped receipt to motion's serial before this library's exact modal
lookup. All 36 competing controls after the faults are refused. The controller's
`2026-09-30-motion-progress` evidence retains 282 raw files and seven rehashed
semantic challenges. Linux shim tests pass 289 with four ignored; Clippy,
shipping build and the standalone C++ snapshot checks pass.

This is commanded-output provenance, not encoder evidence or restart authority.
Shim tests at that checkpoint covered pending-but-unexecuted abort and
brake-boundary crossing; the later motion-birth checkpoint above qualifies the
first through the actual task and binds commands to a specific motion lifetime.
Braking-boundary crossing still needs its full-stack test.

The preceding accepted-receipt projection is compiler/runtime
`68fe04696876706e53b6496f0406d34b369892bb`, paired with controller
`49c35409f5930c76ed5f219916f6f28aebce1556`. **24 simulator sessions pass**:
18 missing/invalid/legal-but-wrong receipt cases and six normal/abort controls on
the mill and lathe. Six baseline sessions reproduce incorrect mode/feed/plane
handoff; the repair refuses 108 competing controls while retaining abort/OFF.
Rust projects expected modes during preparation and serves constant-time lookups
only inside the accepted, not-yet-drained motion-piece prefix. Stop revokes live
authority while retaining that accounting; the task compares exact dispatch
serial, source ordinal and modes before recovery. Ten Linux release ABI tests
pass in the new image; 38 local Rust task/FFI tests, formatting and Clippy pass.
The controller's `2026-09-30-exact-receipts` checkpoint preserves 486 raw files,
source/component identity, geometry/state audits and rehashed evidence challenges.

Acceptance lookup alone is not execution evidence; the motion-progress checkpoint
above supplies a separately checked motion record. The preceding
`2026-09-30-stopped-receipts` checkpoint passed 14
sessions by refusing ambiguous recovery instead of guessing pre-motion modes.

The companion controller's task-local start-authority checkpoint is
`101c2f6e3a9a3aa320c4b6bb5fc519b68244fe76`. **12 simulator sessions pass** after
reproducing unintended motion from a receipt-named G-code file on both machines
following task replacement. Ordinary AUTO now requires successful selection in
the current task process; native AUTO retains this Rust library's owner gate.
Twelve RUN/STEP/RESUME probes refuse starts before fresh loading or after failed
ordinary replacement. Fresh ordinary runs/reruns, native/mixed jobs, hold/resume,
semantic stepping, queued aborts and loading regressions pass. The controller's
`2026-09-30-start-authority` evidence retains 281 passing-run, 41 baseline and 37
harness-error raw files. Compiler code is unchanged.

The preceding `2026-09-30-restart` checkpoint at controller
`8d8092344b9ad2732a6f7f8239129937b5aa7f8d` passed eight sessions, including
consecutive jobs and graceful drained task replacement with motion alive and
explicit private fixture I/O reconnection. It repaired owned I/O cleanup and
zeroed startup limit caches. Its earlier absent-file start probe was insufficient;
the collision checkpoint above provides the stronger refusal evidence.
Neither checkpoint qualifies task crash/connection-loss stopping, restart during
motion, lost/corrupt receipts, or every close/reset/entry/transfer route.

The earlier execution-identity/mixed-sequence checkpoint
is `27e457fd8551ded316e1aa7cde3a7269997e8366`. **25 simulator sessions pass**:
two mixed sequences containing six native jobs and four ordinary RS274 AUTO
programs, six state regressions, 15 zero-tool pipelines and two loading checks.
Native compatibility tags add a fresh execution discriminator to the selection
counter; independent task processes no longer publish identical recovery names.
The C++ bridge creates it before Rust execution ownership starts; no Rust ABI or
servo changes are needed. Exact evidence is the controller's
`2026-09-30-sequences` checkpoint, with 542 raw files, the two baseline namespace
failures, and 28 files from a corrected harness mistake. Source/geometry/state
audits and three semantic corruption challenges pass.

That checkpoint qualifies mixed sessions and namespace separation; the later
checkpoints above add consecutive jobs and drained task replacement.
At that checkpoint, the fallback to pre-motion modes on an unmatched receipt
remained unproven; a new namespace did not resolve it. The later stopped-receipt
repair above removes that inference. Compiler code at the sequence checkpoint
was `ebcae527ac61099cc3ad55d8544f2323036e594f`.

The preceding zero-tool/state-publication build is
`af2eb0d723997d73f85d09010cbaafa7e13d5466`, with this compiler unchanged at
`ebcae527ac61099cc3ad55d8544f2323036e594f`. **41 simulator sessions pass**:
15 zero-tool native/RS274/G7/G8 pipelines, 14 execution/shaper/freshness
regressions, ten earlier coordinate pipelines and two loading/G-code sessions.
Native state now distinguishes active G43 with a zero H offset from G49, and
publishes accepted interpreter modes after a verified drain. Canonical work
coordinates retain their original millimetre bits, preventing false held-resume
refusals after inch conversion. Exact Rust fingerprints remain unchanged.

The controller's `2026-09-30-zero-tools` evidence retains 738 successful-run raw
files and both the original and intermediate failed attempts (248 and 250 raw
files). Offline audits rederive geometry, offset modes, held positions and state
handoff; corruption challenges require semantic rejection despite fresh hashes.
Only the task executable changed across these runtime images. Zero-H abort/fault
combinations, multiple-job/task-restart receipt identity, receipt loss, stationary
records, full-stack braking crossings and the remaining
coordinate/procedure/termination/entry/fault matrix stay open.

The preceding, separately pinned queued-recovery build is
`c7536114b37818ba596efc9692f41bcefa8cd5f5`, with this compiler unchanged at
`ebcae527ac61099cc3ad55d8544f2323036e594f`. It passes **36 simulator sessions**:
queued and single-cut state/abort cases, later plane/feed/direction transitions,
lifecycle/shaper regressions, ten same-table native/RS274 coordinate pipelines
and two loading/G-code regressions. The failed baseline restored a future queued
G1/F480 instead of the stopped F120 arc. The Rust shim now retains the final
resolved native tag at rest, and task reconciliation restores the executed
motion mode, plane and feed directly before cleanup. No generated G-code is used.

The Linux shim suite passes 284 tests with 4 ignored and Clippy. A planner test
crosses a move boundary while braking without intermediate task observations;
the full-stack later-cut tests instead abort inside the selected later arc.
The controller evidence retains 647 successful-run and 38 baseline raw files,
16,807 servo samples, 228 source-input hashes and semantic corruption challenges.
See the controller's `2026-09-30-queued-recovery` evidence and
[PR #1365](https://github.com/markmounteer/linuxcnc/pull/1365).
Neither checkpoint completes Stage 3 or qualifies physical-machine execution.

The current implementation is the safe Rust
[`nextnc-task` library](../../../crates/nextnc-task/README.md). It has 27 tests:
one semantic-group test, seven binding tests over a 26-fixture geometry matrix,
eight lifecycle tests, six dispatch-receipt tests and five lowering tests. The
[`nextnc-task-ffi` crate](../../../crates/nextnc-task-ffi/README.md) adds nine ABI
tests. These are model/unit tests,
not task integration or physical-machine qualification.

Code checkpoint `812f781fb1cef71dc1114dad5dfef7d06985891f` passed
[all 11 hosted jobs](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36745525139).
[ci.json](ci.json) preserves the exact run/job identities and conclusions. The
Windows full Rust suite passed 109 tests; formatting, Clippy, inventory checks
and an ARM64 compile check also passed locally. ARM64 execution is untested.

The subsequent ABI/lowering code checkpoint
`47e8cb338202187fcbe159fe8f8ef824c1ffdcf8` passed
[all 11 hosted jobs](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36748931127)
and 116 local Windows Rust tests, formatting, Clippy and inventory verification.
The companion pinned-header container passed 52 representation cases (26 source
fixtures in mm/inch external units) with full traces. This is cross-language
preparation/NML evidence, not LinuxCNC task execution. [abi-ci.json](abi-ci.json)
preserves this run separately from the earlier component checkpoint.

The latest lifecycle ABI joins immutable candidates to the owner and expanded
message ledger, with thread confinement, live snapshot fingerprints, generation
checks, hold/step controls and explicit issue/result/drain accounting. The local
Windows workspace passes 119 Rust tests, formatting, Clippy and the validation
inventory. The companion task-loading simulator passes XYZ mill and XZ lathe
sessions using real task state: native selection, competing command refusal,
worker release, invalid replacement refusal and ordinary G-code after cancellation.
No native motion is issued by that checkpoint; full execution remains below.
Code checkpoint `4b8cf0bcae4f4f1c7e42d4df4f2769136e3d5eb4` passed
[all 11 hosted jobs](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36753821315).
[owner-ci.json](owner-ci.json) records the exact jobs. The simulator separately
passed six release-profile ABI tests on Linux before its two real task sessions.

Execution checkpoint `318b9782f10f2763aa3b092cc41d40d8ffa799a4` passed 121 local
Windows Rust tests, formatting, Clippy, validation inventory and
[all 11 hosted jobs](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36759414313).
[execution-ci.json](execution-ci.json) preserves the exact run and job identities.
The companion controller at `e6014632708d8fe0944fcd54a96fce33c85d7f88` passed 19
native task/motmod/shim simulator sessions and two loading/G-code regressions,
plus seven Linux release ABI tests. The 19 include all 13 millimetre geometry
fixtures and hold/resume, held abort and confirmed stepping on both machines,
with implicit-motion MDI handoff afterward. A source/setup oracle checks the
ordered actual motion trace. Full Stage 3 qualification remains below.

| Stage 3 requirement | Current evidence | Remaining acceptance gate |
| --- | --- | --- |
| Rust task owner and checked native ABI | Preparation, lifecycle/dispatch/procedure-rebind ABI and typed lowering; real recipient dispatch | Full fault injection and wider procedure outcomes missing |
| Pinned task executable | Native-bound task executes initial mill/lathe jobs in isolated simulation | Full execution acceptance matrix pending |
| State capture/reconciliation and `on_abort` ordering | Typed updates, cached modes, executing tags, queued-state recovery and ten observed cleanup cases | Zero-valued active H, restart/receipt-loss identity, full offset/unit/modal and abort-race matrix remain open |
| One start gate, drain/result/rebind | Central gate, actual drains and tool-result suffix rebind; changed H2 and invalid/replaced suffix tests | Full NML/HALUI/pendant and start/hold freshness coverage missing |
| Semantic stepping and no arbitrary restart | Versioned groups, actual hold-to-step proposal/confirmation and drain | Full held/read-ahead/corner behavior matrix pending |
| Shaper mode and delayed completion | Bounded lane comparison fixes CW-to-Z fault; 15 execution/preflight/guard cases and two loading regressions pass, with cutting-arc tail hold/abort | Wider unit/state/fault combinations remain part of full qualification |
| Same-table G7/G8/native coordinate matrix | Ten real native/unchanged-RS274 MDI pipelines across four mm/inch/rotation/T1-H2/G7/G8 cases | Complete coordinate and abort-state matrix missing; these sequential MDI references do not compare throughput |
| Termination conditions and ordered state deltas | Rust lowering retains ordered events and emits termination deltas with drain barriers | Actual pending-boundary behavior remains untested |
| Receipts, backpressure, identity, connection loss | Model plus actual queue/guarded-mailbox traces; undersized expansion refuses before selection | Full backpressure/retry/fault injection and disconnect stop tests missing |
| Lines, planes, full circles, helixes and events | Initial native synthetic jobs through real task/motmod/shim with a source/setup oracle | Full state/event/job corpus remains pending |

Procedure checkpoint `ab7442e9ac469f86306d03e5d9ed6ffe7d47120a` passed 122
Windows Rust tests, formatting, Clippy, the validation inventory and
[all 11 hosted jobs](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36763451270).
[rebind-ci.json](rebind-ci.json) preserves their exact identities. The companion
controller `b80d30f75836339396071b7ab7cd24d3df07c083` passed 27 native sessions,
two loading/G-code regressions and eight Linux release ABI tests. The native
matrix reruns all earlier cases and adds changed H2, out-of-limit H2, replaced
bundle and tool cancellation for both machines. These are still mm/G94 sessions
with shaping disabled, and do not complete Stage 3.

The enabled-shaper follow-up used the same production binaries with a dedicated
servo sampler. Twelve of thirteen sessions passed; the clockwise XY arc's Z
retract caused a false mixed-axis fault. A 17-digit capture shows a rounding-scale
XY change immediately before the exact-equality lane check selects shaping.
Cutting-arc tail hold passes. Cutting-arc tail abort drains, but its follow-up
test wrongly assumes linear MDI instead of the correctly retained G3 mode.
The companion controller preserves both runs in
`controller/motion/motion/tests/nextnc-stage3/evidence/2026-09-30-shaper-transition-failure/`.
This checkpoint is failed qualification, not a completed shaper gate.

The subsequent controller repair at `32c200fad8be785eddfe9bdfb2e5d74efdbc10c3`
passes 15/15 execution, preflight and downstream-guard cases plus 2/2 loading
regressions. It admits only bounded numerical residue against the fixed arm
pose; actual RS274 mixed lines/helices remain rejected by motmod. Cutting-arc
hold/abort and retained G3 handoff pass. Full build `dc3670b0ecdb0f473f0fc7e05075c5f17afe12f4`
produces identical runtime binaries to the differential test image. Only motmod
changes from the previous runtime; Rust compiler/ABI code is unchanged.
The separate `2026-09-30-shaper-boundary` evidence folder contains 305 raw files
and its own verifier. Earlier failure evidence remains intact.

The start/resume checkpoint at compiler `ebcae527ac61099cc3ad55d8544f2323036e594f`
and controller `4df1f22348402043412eb39ff170c53c28351b03` passes 60 simulator
sessions: 32 normal cases, 16 deterministic worker races, two loading/G-code
regressions and ten repeated holds. Rust checks selected-source identity and
bound inputs; the task rechecks live state before adoption. Late results lose
authority after PAUSE, ABORT or replacement. Changed cached NEXTNC policy is
refused with a restart diagnostic. Tool confirmation still requires actual
result/drain/suffix rebind. The companion controller preserves both successful
and earlier failed runs under `2026-09-30-start-resume`, with an offline verifier
and semantic corruption challenges. Full Stage 3 remains incomplete.

At compiler `ebcae527ac61099cc3ad55d8544f2323036e594f`, the workspace passed
123 Rust tests, formatting, Clippy and the 259-site
inventory. Nine Linux release ABI tests pass in the task image. Documentation
checkpoint `cd61bae17e42895e18c8b1bb226fc500a8c4dbf7` passes
[all 11 hosted jobs](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36772650184),
recorded in [freshness-ci.json](freshness-ci.json). An ARM64 workspace/all-targets
compile check passes; ARM64 execution is untested.

Next are the complete coordinate/modal, procedure, termination, entry-route and
fault/disconnect tests, including remaining freshness combinations. This work
must keep the existing task and motmod guards. Appending an
NML message is not a guarded-motion receipt; planner-done is not shaper-done.
Source/operation identity must survive any one-to-many message expansion.

The reference simulation already passed 21 repeated admission cases and 28
baseline cases, with shaping disabled. It is not evidence of native execution,
engaged shaping, G95/CSS, manual tool confirmation or safe deployment. G95/CSS
execution remains Stage 4. No controller was contacted or modified for this work.
