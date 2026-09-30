# Stage 3 progress — native task execution is not accepted

Date: 2026-09-30. **IN PROGRESS. No native job has reached LinuxCNC through a
qualified task adapter.** The completed Stage 0 simulator reference and
[Stages 1–2](../stages12/README.md) remain distinct evidence.

The current implementation is the safe Rust
[`nextnc-task` library](../../../crates/nextnc-task/README.md). It has 26 tests:
one semantic-group test, seven binding tests over a 26-fixture geometry matrix,
eight lifecycle tests, six dispatch-receipt tests and four lowering tests. The
[`nextnc-task-ffi` crate](../../../crates/nextnc-task-ffi/README.md) adds three ABI
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

| Stage 3 requirement | Current evidence | Remaining acceptance gate |
| --- | --- | --- |
| Rust task owner and checked native ABI | Owner model, checked preparation/read ABI and typed lowering implemented | Lifecycle ABI and actual task binding missing |
| Pinned task executable | Qualified reference image source available | Patched native task binary and identity missing |
| State capture/reconciliation and `on_abort` ordering | Model withholds MDI until explicit reconciliation | Actual canonical/interpreter restoration and abort-race tests missing |
| One start gate, drain/result/rebind | Model tests generations, competing start, readiness and prefix barriers | NML/HALUI/pendant/mode-switch and tool procedure integration missing |
| Semantic stepping and no arbitrary restart | Versioned groups and model tests | Actual held/read-ahead/corner behavior untested |
| Shaper mode and delayed completion | Geometry refusal, lane-change drains, missing-tail model tests | Engaged-shaper full-stack runs missing |
| Same-table G7/G8/native coordinate matrix | XYZ/XZ mm/inch synthetic binding with nonzero offsets and T1/H2 | Independent RS274 comparison and real state capture missing |
| Termination conditions and ordered state deltas | Rust lowering retains ordered events and emits termination deltas with drain barriers | Actual pending-boundary behavior remains untested |
| Receipts, backpressure, identity, connection loss | Model accounts for expanded messages, actual-result categories, retry and unknown-result refusal | Real queue/guarded-mailbox traces and disconnect stop tests missing |
| Lines, planes, full circles, helixes and events | Source fixtures and continuous machine-path checks | Native full jobs through real shim missing |

The next implementation boundary is connecting lifecycle dispatch and the typed
messages to the pinned task. It must keep the existing task and motmod guards. Appending an
NML message is not a guarded-motion receipt; planner-done is not shaper-done.
Source/operation identity must survive any one-to-many message expansion.

The reference simulation already passed 21 repeated admission cases and 28
baseline cases, with shaping disabled. It is not evidence of native execution,
engaged shaping, G95/CSS, manual tool confirmation or safe deployment. G95/CSS
execution remains Stage 4. No controller was contacted or modified for this work.
