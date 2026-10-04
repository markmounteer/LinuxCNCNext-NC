# Stage 1–2 acceptance: source contract and offline Rust compiler

**PASS, 2026-09-30: Stage 1 and Stage 2 requirements are complete for the
supported Next-NC profiles.** This accepts the producer contract and offline
compiler. It does not accept a native execution adapter, synchronization,
controller deployment or physical machining. Every native CLI result still says
`executable:false`.

The acceptance authority is the unchanged
[Stage 1 and Stage 2 plan](https://github.com/markmounteer/linuxcnc/blob/4a994d56012b7f024ee44015960ea39003b6dcba/controller/motion/motion/docs/nextnc-native-optimization-plan.md#1-version-the-semantic-contract-and-preserve-source-geometry).
The following review maps each requirement to implementation and exercised
evidence; completion is not inferred from output-file existence or a checklist.

## Qualified revisions and checks

| Component | Exact revision | Evidence |
| --- | --- | --- |
| Fusion producer | `b31904b3d5ad9d56cc4cc16b8090e768b0e1958d` | [9/9 hosted jobs](https://github.com/markmounteer/Fusion360Next-NC/actions/runs/36676034052): 84 producer tests, old-reader refusal and current-CPS/Rust comparisons on Linux/Windows, generated-CPS verification and Windows diagnostic collector |
| Rust compiler | `a31dd543e917cd314e7806ede7e099bc7a332f3f` | [11/11 hosted jobs](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36697982337): 87 Rust tests on Windows, 88 on Linux including the Unix FIFO test; formatting, Clippy, inventory and benchmark smoke checks; existing Node/conformance/interpreter lanes |
| Shared type source | `34e586f66b4f17de8edd0ed9514a428ab7905db3` | Controller facade imports this exact public revision. `git diff` to the qualified compiler shows no changes under `crates/motion-command`; no duplicate type definitions |
| Reference simulation | Harness `9b2df9144c8d6662af0756548df138dc8868ed03`; evidence at controller `4a994d56012b7f024ee44015960ea39003b6dcba` | [21/21 reference admission cases and 28/28 same-image baseline cases](https://github.com/markmounteer/linuxcnc/blob/4a994d56012b7f024ee44015960ea39003b6dcba/controller/motion/motion/tests/nextnc-stage0/evidence/2026-09-30-admission/README.md), raw archives, identity manifests and complete trace audits |

[ci.json](ci.json) preserves the queried run identities/job conclusions. The
Windows and Linux Rust totals comprise respectively 64/65 compiler tests and
23 shared-contract tests. Tests contain corpus and mutation loops, so these
totals are test functions, not individual input counts. Existing Node tests pass
105/105 on Linux and 104 with one POSIX-only skip on Windows. Controller-hosted
Actions are disabled; no hosted controller result is claimed or substituted for
the saved desktop simulator evidence.

Additional final checks at the compiler revision: release build and
`aarch64-unknown-linux-gnu` compile check passed, the inventory verifier passed,
and all ten saved Autodesk-post outputs again matched complete Rust models and
fingerprints while the revision-1 reader refused them. All seven producer source
hashes in the native fixture manifest still match the current Fusion checkout.
ARM execution is untested. Actual Autodesk engine output is captured evidence;
this final review reused it rather than claiming another engine invocation.

## Stage 1 requirement review

| Plan requirement | Implementation and acceptance evidence | Result |
| --- | --- | --- |
| Version profile/post and contract together; preserve old-profile reading with explicit missing fields | [Native capability matrix](../../native-capability-matrix.md), revision-2 producer properties and `motion-command::v2`; four revision-1 model/fingerprint fixtures retain missing tolerance and unspecified movement | PASS |
| Tolerance provenance, movement class and source-required capabilities | `profile.rs`, `capabilities.rs`, `tests-rust/profile.rs`; full ordered 130-case model comparisons, closed-vocabulary/forged-version/missing-capability tests | PASS |
| Independent direction, sweep and signed rise; principal-plane conventions, full circles, multiple turns, inch/mm, XYZ/XZ | [Geometry tests](../../../tests-rust/geometry.rs), contract v2 tests, native profile corpus; analytic continuous-path bounds and independent path metrics, including 120 synthetic and ten Autodesk outputs | PASS |
| Whole-job unsupported cycles, phase threading, probing and rotary refusal | Closed profile graph/shape/requirements validation, post prechecks and negative fixtures; compiler-added capabilities checked again on the complete prepared plan | PASS |
| Cross-layer capabilities and backend-independent process state | Capability matrix explicitly separates producer, reader, contract and later live admission; complete spindle/feed/coolant/tool/dwell/event ordering tests | PASS |
| Actionable through-tool coolant diagnostic without substitution or duplicate machine options | Producer native-profile/precheck tests and Rust `COOLANT_CAPABILITY` rejection; no Flood/Mist fallback | PASS |
| Producer/consumer release matrix and semantic fingerprints; v1 refuses v2 semantics | Producer CI compatibility lanes; 130 native and four legacy Rust fixtures; all ten actual posted outputs matched again in the final review and were rejected by the old reader | PASS |
| Continuous path, feed dimensions and event order agree; no flattened/mislabelled helix | Complete ordered models plus independent geometry tests; revision-2 helix stores sweep/rise explicitly and has no planar `basic curve`; serialized command audit retains analytic geometry | PASS |

The supported format is a closed experimental Next-NC profile, not general
AP238 certification. A source capability declaration is not installed-controller
evidence. G95 and CSS remain dimensioned requirements; neither is substituted
with a feed calculated from nominal RPM.

## Stage 2 requirement review

| Plan requirement | Implementation and exercised evidence | Result |
| --- | --- | --- |
| Rust parser, semantic checks, inspection, diagnostics and source map; no Node in native runtime | `src-rust` CLI/library; standalone tests clear environment/PATH for preflight, preparation, publication, reload and demand analysis; Node remains development oracle only | PASS |
| One version-pinned shared contract, no forked motion types | Public `crates/motion-command` source; controller facade re-exports exact Git dependency, unchanged at final compiler revision | PASS |
| Port profile and execution-plan semantics before optimization/executor | Four complete legacy command/waypoint/event/feed projections, all 130 native profiles, separate `command_audit` geometry/completeness/policy checks; exact-path baseline | PASS |
| Separate semantic state/provenance generation from serialization | Immutable `PreparedPlan` is constructed only after audit; `bundle.rs` serializes and independently decodes actual records rather than trusting the in-memory source plan | PASS |
| Bound input bytes, entities, references, depth, expansion and commands | `part21::Limits`, graph/shape/JSON/geometry expansion and command/bundle bounds; malformed inputs, each grammar arity/attribute, late failures and at/over-limit tests; Unix FIFO refusal | PASS |
| Measure large arc-heavy jobs before raising limits or changing representation | [Matched benchmark](../../benchmarks/2026-09-30-windows/comparison.md): ten jobs, including 34,000-arc mill/lathe jobs and 8,500 mill helices, unchanged limits; 90 measured processes/540 timed calls per revision | PASS |
| Intern metadata, semantic state deltas and bounded diagnostic lookup without losing ordered uses | Spans, explicit process-state actions and command source-use tags; repeated/backtracking vertices retained; every command deletion/replacement and provenance mutation audited; full command/span digests retained | PASS |
| Whole job validated before any motion; unsupported suffix cannot execute partially | All CLI commands are offline/nonexecuting. `prepare` validates and audits the entire job; late profile/setup/tool/capability failures produce no successful candidate; bundle independent audit repeats the checks | PASS for compiler; live enforcement belongs to Stage 3 |
| Key artifacts by source/schema/compiler/policy; never cache live permission | Bundle exact input/build/policy identities, optional-snapshot distinction, altered identity and command rejection tests; no execution cursor/live binding in artifacts | PASS |
| Staged publication and selection generations | OS-locked owner, opaque worker ticket, new generation, verified staged file, independently validated immutable object; no restored selection on restart | PASS |
| Failed replacement, crash/cancel, stale worker, source mutation, corrupt bundle and full storage | Publication tests exercise six real process-exit checkpoints, injected partial/storage failures, cancellation before/after commit, old workers, source/setup/table changes, independent bundle reuse and corruption; no old-selection fallback | PASS |
| Exact reductions first | [Reduction invariants](../../native-exact-reductions.md): span interning, event deltas, streamed canonical hash, moved provenance and analytic helices; materialized hash oracle and complete ordered command/span comparisons | PASS |
| Fit/blend accounting | [Error report](../../native-error-budget.md): allocated/used fit and blend both zero; CAM tolerance kept separate; absent post/continuous numeric/total error bounds stay null; forbidden-policy and CLI archival tests | PASS; no positive geometric allowance enabled |
| Rate warnings from measured reference and predicted demand, including state and expansion | [Demand report](../../native-command-demand.md), six unit tests, CLI/archive/invalid-evidence tests, all native fixture command counts and the six release-job results below; complete 21-case reference validated | PASS for offline screening; native ingress/piece capacity remains explicitly unqualified |
| Keep unoptimized first execution baseline; measure existing coalescing/blending before a combiner | Exact-path policy remains unchanged. Stage 0 direct-shim replay retained its measured zero/negative gains; no upstream segment combiner is introduced | PASS |
| Keep analytic helices rather than old-reader linearization | Producer revision 2 and Rust bundle preserve continuous geometry; large-job release analysis counts all 8,500 multi-turn mill helices | PASS |
| Determinism, malformed/resource failures, full ordered semantics, geometry/event equivalence and benchmarks | Binary determinism, rechecksummed-corruption rejection, all-order model/command tests, limit tests, matched benchmark complete command/span identities; repeat release analysis gives identical demand reports | PASS |
| Disposition every existing check; refuse admission if a required check is missing | [Inventory](../../native-validation-inventory.md): 259 sites, 61 arities, 187 attributes, seven combinations, all 32 module scopes, with source/test hashes and CI drift verifier; no required compiler check remains marked missing | PASS; no blanket full-AP238 or runtime parity claim |

The resource caps are bounds, not a capacity promise. The largest measured jobs
still use substantial RAM (about 1.3–1.4 GiB peak preparation RSS in the matched
workstation run). No Pi-memory, hard real-time or power-loss qualification is
inferred. Crash/storage-fault tests qualify the stated software lifecycle.

## Final release CLI demand qualification

[rate-results.json](rate-results.json) records the executable, source, setup,
reference, artifact and runner hashes. Each case publishes a complete bundle,
runs `analyze-rate` twice with PATH empty, verifies the exact repeated demand
report, confirms the artifact was unchanged, checks every motion/state/waypoint
count against the independent input manifest and published audit, and requires
saved diagnostics. Raw outputs are retained under [raw](raw).

| Synthetic input | Motions | State actions | Reviewed waypoints | Analytic helices | Expected diagnostic |
| --- | ---: | ---: | ---: | ---: | --- |
| Mill mm arcs | 34,000 | 123 | 10 | 8,500 | Finite nominal feed density; no capacity claim |
| Lathe mm arcs | 34,000 | 123 | 6 | 0 | All 34,000 G95 durations unknown |
| Mill inch arcs | 1,000 | 24 | 10 | 250 | Canonical dimensional timing |
| Mill mm polyline | 34,000 | 123 | 10 | 0 | `NOMINAL_DENSITY_ABOVE_REFERENCE` |
| Lathe inch arcs | 1,000 | 24 | 6 | 0 | All 1,000 G95 durations unknown |
| Lathe mm polyline | 34,000 | 123 | 6 | 0 | All 34,000 G95 durations unknown |

The mill polyline's peak ideal density is about 209,836 prepared actions/s. That
is a mathematical demand screen, not a feasible machine speed or a proven native
starvation rate. States count in its numerator; procedure waits, acceleration,
jerk and spindle waits are not predicted. Measured reference mailbox delivery
includes different producer/planner delays. Adapter call expansion and downstream
pieces remain null until Stage 3; compiler polyline expansion is already exact.
No warning changes source geometry, feed or tolerance.

To reproduce, use the qualified compiler and the producer revision above, then:

```text
node tools/native-benchmark/generate.cjs PATH_TO_FUSION NEW_INPUT_DIRECTORY
python tools/native-benchmark/qualify-rate.py NEW_INPUT_DIRECTORY NEW_OUTPUT_DIRECTORY
```

The generator is development tooling. The measured/checked executable is Rust
only. The runner builds it, refuses modified compiler sources, creates a new
output directory, validates input hashes and retains failures. Compare counts,
fingerprints and scoped conclusions across platforms; build/target-dependent
artifact hashes and process timing need not match. Process durations include
reload, validation and diagnostics, and are not the earlier core benchmarks.

## Next stage and explicit limits

Stage 3 must implement a task-owned Rust AUTO execution source beside the
unchanged RS274 interpreter, bind fresh machine/tool/offset/capability state,
send typed commands through guarded admission, and qualify start ownership,
hold/resume, single-block, abort, backpressure, stale generations and complete
planner/shaper drain in the real desktop simulator. Start with one line, arcs in
each principal plane, full circles, a helix and ordered machine events. Ordinary
MDI/probing remains with the existing interpreter. GUI/preview work is outside
the requested scope.

Stage 4 must implement and qualify true spindle synchronization and CSS for
the lathe. The existing G95/CSS simulation evidence is refusal evidence, not
lathe-job execution equivalence. Engaged input shaping, physical tool changes,
machine clearances, deployment and physical acceptance also remain later gates.
No controller was contacted, modified or moved during this qualification.
