# Stage 3 progress — native task execution is not accepted

Date: 2026-09-30. **IN PROGRESS. Native jobs now execute in the experimental
LinuxCNC task simulator; full Stage 3 acceptance remains incomplete.** The completed Stage 0 simulator reference and
[Stages 1–2](../stages12/README.md) remain distinct evidence.

The current implementation is the safe Rust
[`nextnc-task` library](../../../crates/nextnc-task/README.md). It has 27 tests:
one semantic-group test, seven binding tests over a 26-fixture geometry matrix,
eight lifecycle tests, six dispatch-receipt tests and five lowering tests. The
[`nextnc-task-ffi` crate](../../../crates/nextnc-task-ffi/README.md) adds seven ABI
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

| Stage 3 requirement | Current evidence | Remaining acceptance gate |
| --- | --- | --- |
| Rust task owner and checked native ABI | Preparation, lifecycle/dispatch ABI and typed lowering; initial real recipient dispatch | Procedure result/rebind transport and full fault injection missing |
| Pinned task executable | Native-bound task executes initial mill/lathe jobs in isolated simulation | Full execution acceptance matrix pending |
| State capture/reconciliation and `on_abort` ordering | Typed canonical/interpreter updates, normal/abort handoff and deferred cleanup implemented | Full offset/unit/modal matrix and abort-race tests missing |
| One start gate, drain/result/rebind | Models plus actual task loading gate, refused starts/mode changes and failed replacement checks | Execution-phase NML/HALUI/pendant, procedure and drain integration missing |
| Semantic stepping and no arbitrary restart | Versioned groups, actual hold-to-step proposal/confirmation and drain | Full held/read-ahead/corner behavior matrix pending |
| Shaper mode and delayed completion | Geometry refusal, lane-change drains, missing-tail model tests | Engaged-shaper full-stack runs missing |
| Same-table G7/G8/native coordinate matrix | Synthetic geometry matrix plus real task capture with nonzero WCS/G92/H2 and lathe G7 | Independent executed RS274/native comparison and full state matrix missing |
| Termination conditions and ordered state deltas | Rust lowering retains ordered events and emits termination deltas with drain barriers | Actual pending-boundary behavior remains untested |
| Receipts, backpressure, identity, connection loss | Model accounts for expanded messages, actual-result categories, retry and unknown-result refusal | Real queue/guarded-mailbox traces and disconnect stop tests missing |
| Lines, planes, full circles, helixes and events | Initial native synthetic jobs through real task/motmod/shim with a source/setup oracle | Full state/event/job corpus remains pending |

The next implementation boundary is procedure result/rebind transport and the
remaining full-stack qualification matrix. It must keep the existing task and motmod guards. Appending an
NML message is not a guarded-motion receipt; planner-done is not shaper-done.
Source/operation identity must survive any one-to-many message expansion.

The reference simulation already passed 21 repeated admission cases and 28
baseline cases, with shaping disabled. It is not evidence of native execution,
engaged shaping, G95/CSS, manual tool confirmation or safe deployment. G95/CSS
execution remains Stage 4. No controller was contacted or modified for this work.
