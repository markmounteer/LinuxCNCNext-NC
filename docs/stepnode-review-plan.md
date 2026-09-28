# STEPNode: review and improvement plan

Status: steps 1–3 implemented for v0.7.0 on 2026-09-28. See [command/state records](command-state.md) for the delivered contract and [validation](validation.md) for test evidence. The original review and pinned baseline follow; SDK integration and other deferred research remain outside this release.

## Recommendation

Use [steptools/STEPNode](https://github.com/steptools/STEPNode), the fourth execution-side repository in the earlier ranking, as an architectural reference for source-aware process traversal and controlled command emission. Build on LinuxCNCNext-NC v0.6.0 with three bounded additions:

1. Trace successful translations back to exact STEP entities and execution-plan fields.
2. Validate internal command shapes, coordinate frames and emission lifecycle.
3. Check that every required source movement and reviewed transition was emitted once, in order.

These changes apply to both **XZ lathes and fixed-axis XYZ mills**. They require no new machining options, controller settings, SDK dependency or CPS behavior change. They improve diagnosability and evidence of translation completeness; they do not promise shorter cutting time or reduced G-code size.

## Verified baseline and review limits

- STEPNode default branch: `master`, commit `220fe390ff7c316d5a304e1485a870758ae06e69`, dated 2019-07-05; tree `bd6c24d7e68ccbf04452ed1288f83fc5b83b22c1`. GitHub marks it archived. Its README says development moved inside STEP Tools. [S1]
- LinuxCNCNext-NC: v0.6.0, local and remote `main` at `2f2c1436654a1f808fbeea329e6628aa944ce0d8`; clean before this review. The shared Fusion reader was checked at `cfda07768f09554148db7b2049d3ad608149c91e`.
- Captured all 36 non-Git-metadata, non-example-data files in the pinned upstream tree. Reviewed the cursor/formatter/state interfaces and their implementations, Finder behavior, generation example, README/build configuration, license and test scaffolding. Downloaded bytes were checked against their Git blob hashes. The manifest and source snapshot are under `D:\dev\Next-Nc research\github-translators-2026-09-28\evidence\steptools--STEPNode\detailed-review\`.
- Reran the target Node suite: **30 passed, one POSIX-only test skipped on Windows, zero failed**. Ran the three isolated formatter probes described below, using only in-memory synthetic commands.
- STEPNode and its external SDK were **not built or run**. The SDK implementation and contemporary SDK availability/licensing were not established. Native LinuxCNC tests were not rerun for this documentation-only review; the existing v0.6.0 validation is recorded in [validation.md](validation.md).
- No private job files were uploaded, and no controller, LinuxCNC configuration or physical machine was changed.

## What STEPNode actually contributes

STEPNode is a native Node wrapper around STEP Tools C++ APIs. `Adaptive` exposes a process cursor; `Generate` formats events; `GenerateState` tracks formatter state. The actual traversal and formatting algorithms delegate to external `StixCtlCursor`, `StixCtlGenerate` and `StixCtlGenerateState` implementations. This repository does not contain a verified, self-contained LinuxCNC backend. [S2, S3, S4]

The build instructions refer to Node 8, Python 2.7 and the external `stdev_core`, `stdev_stpstack` and `stdev_ncstack` packages. The repository has an Apache-2.0 license; that does not establish redistribution rights for the separate SDK. Independently authored Node changes are the practical path here. [S1, S2]

The useful interfaces expose project/operation/path boundaries, moves, errors and completion; active STEP object identities; positions with coordinate-system and source-object information; and explicit state invalidation. These are useful design references, not evidence that STEPNode accepts our Next-NC profiles or correctly translates our lathe/mill jobs. [S3, S4]

The example `generate.js` selects an `okuma` style, writes placeholder program/tool-change text and appends demonstration moves to `(900,950,999)` and `(1900,1950,1999)`. It writes directly to stdout and has no explicit `ERROR` case. The wrapper exposes `ERROR` and `ErrorMsg`, but the external formatter's behavior on that event was not inspected. This example demonstrates API plumbing, not a production translation policy to copy. [S5]

`SetWantedAll` controls event reporting. Separately, `SetVisitAllExecs` controls whether disabled and alternative executables are visited; its default can select one enabled alternative. Those controls must not be conflated or adopted as permission to silently choose machining operations. Our bounded profiles continue to reject unsupported executable structures. [S3]

## What v0.6.0 already has

The preceding NIST review already led to structured commands, per-translation modal state, state changes and motion snapshots, geometry-error provenance, and independent LinuxCNC semantic comparisons. The current translator also has local diagnostic archives, HTML reports, aggregate plan checks, tool-table checks, exact number serialization, atomic output creation and lathe/mill profiles. Recreating those features would add little value. [T1, T2, T3]

The remaining differences are narrower:

| Upstream reference | Current target finding | Proposed improvement |
| --- | --- | --- |
| Active workingstep/operation/toolpath IDs and position source-object IDs. [S3] | Geometry failures have entity/line context, but successful models/maps retain section/path/vertex ordinals without the original record chain. [T2, T3] | Add source provenance outside the fingerprinted model. |
| Separate cursor, formatter and state interfaces. [S3, S4] | Command objects already exist, but known command types largely trust their field values and lifecycle. [T1] | Validate the existing internal boundary rather than introduce another command language. |
| Explicit operation/path boundaries, `ERROR` and `DONE`. [S3] | The test helper compares decoded path associations; production translation does not perform an equivalent final completeness audit. [T3, T4] | Add an ordered source-use audit before returning candidate output. |
| State reset methods and coordinate-system metadata. [S3, S4] | Work/machine positions and unknown state already exist; requested-state caching is separately maintained. [T1, T3] | Check consistency while retaining existing M6 assumptions and reset behavior. |

### Reproduced internal contract gaps

Calling `LinuxCNCOutput.emit` directly with synthetic commands at the baseline revision produced:

| Probe | Accepted output |
| --- | --- |
| Initialize mm/XZ, then linear axes `{a: 1}` | `G1 undefined1` |
| Initialize mm/XY, then declare an XZ arc with X/Z endpoint and I/K center | `G2 X1 Z1 I1 K0`, with G17 still active |
| Initialize, end, then rapid X1 | `M2` followed by `G0 X1` |

These probes bypass the public reader/plan validator. Normal validated inputs do not expose this command API, and no current production input failure was demonstrated. The finding is a missing internal contract that could allow a future translator regression to produce invalid commands. Unknown command *types* already throw; the new checks must cover invalid fields and sequencing too. [T1]

## Implementation sequence

### 1. Preserve successful source provenance

Extend the shared Fusion inspector to return an additive provenance sidecar indexed by section, path and vertex use. Keep it outside `model` and the decoded-program fingerprint. Use the existing parser's `doc.locations` and actual graph references; do not rescan text or infer identifiers from names.

Record the owning workingstep and operation, toolpath, curve and relevant geometry records. Polyline segments need both endpoint references; arcs need the trimmed curve, circle/placement and actual endpoint/center sources. Dwell and process-state values should identify their representation/property records. Shared geometry must retain the particular owning path/use as well as the shared entity. Derived values should identify their source inputs and derivation rather than invent an entity for the result.

Carry this information into source-map entries and the HTML report. For translator-added movements, identify the execution-plan JSON Pointer, such as `/sections/0/retract/0`, `/sections/0/approach/1`, `/sections/1/moves/0` or `/end/0`. Identify T/H and WCS mapping fields separately. Header/reset/end policy commands should be labeled as translator policy, not attributed to nonexistent STEP records.

Bind raw record IDs and line/column locations to the exact input SHA-256, and plan pointers to the exact plan hash in CLI archives. Keep the existing candidate G-code hash. Entity renumbering or whitespace changes can alter source provenance without altering the semantic fingerprint or requiring a newly reviewed plan. Do not describe these IDs as permanent Fusion operation IDs across reposts.

Candidate files: `Fusion360Next-NC/lib/inspect.js` and shared reader tests; then a pinned vendor refresh and provenance record in this repository, `src/profile.js`, `src/translate.js`, `src/report.js`, CLI/archive tests and `docs/preflight.md`. Make the shared change upstream rather than maintaining a divergent vendor fork. No CPS change is required.

Acceptance:

- In both machine profiles, a selected output move identifies the exact source path/geometry use, record and one-based line/column; plan movements identify the correct plan field.
- Renumbered, reformatted and shared/unshared synthetic files preserve fingerprints, plan acceptance and G-code while updating source references correctly. Existing tests that require identical entire source maps for shared/unshared geometry should compare execution fields separately from the new provenance fields.
- Full-circle commanded endpoints remain distinct from source endpoints and center-offset derivations. Existing geometry tolerances are unchanged.
- Preserve `source-map/1` with additive optional fields, error codes and older archive rendering. Reports escape source text as they do now and do not embed the entire STEP input by default.

### 2. Enforce the existing command contract

Add a small validator at the `LinuxCNCOutput.emit` boundary, with machine/profile context supplied once by `translate`. Validate before mutating state, lines or source maps. Keep commands internal.

Checks should include known fields and enum values; finite numeric values using current limits; integer tool/H identifiers; allowlisted WCS commands; boolean arc/spindle flags; nonempty legal motion axes; explicit work/machine frame; and plane-appropriate arc endpoint/center words. Require declared arc plane to agree with the emitted modal plane. Preserve the intentionally empty endpoint axes of a full circle. Reject machine-frame arcs, unsupported axes and planar arcs with perpendicular travel under the current profiles.

Validate units/modes established before motion and a lifecycle that allows header comments, the existing initialization and post-M6 reinitialization sequence, and exactly one terminal end. Reject commands after end. A machine retract before the first tool selection is valid; do not accidentally ban the translator's existing entry sequence. Field checks should describe the command index, phase, operation/path and source provenance when available.

Keep internal invariant failures distinguishable from user input failures: use a typed internal error that the CLI archives as `INTERNAL_ERROR`, with an invariant identifier and structured context. Continue preserving existing public geometry/plan errors. A failed command must leave the in-memory emitter unchanged, and any translation failure must still publish no G-code.

Requested-state caching and emitted-state knowledge have different meanings today. Document and test their reset boundaries, particularly M6. Do not equate an unknown state with zero/off, or silently change the current spindle/coolant sequence in the name of validation. A stricter M6 policy would be a separate behavior change with its own machine integration review.

Candidate files: `src/linuxcnc-output.js`, a focused internal validation module if useful, `src/translate.js`, error/CLI tests and a command-contract test file.

Acceptance: the three reproduced probes and invalid units/frame/tool/offset/flag cases fail before emission; existing valid command sequences and exact golden G-code remain unchanged. Interleaved translation instances cannot share state. Exercise mm/inch, XZ lathe, all three mill planes, CW/CCW, partial/full circles, CSS/RPM, G94/G95 and all existing transition modes.

### 3. Audit ordered source consumption before output

Borrow the cursor's explicit boundaries without building a second interpreter or replacing the existing command layer. Enumerate expected source uses from the fully validated model and execution plan: one token per polyline segment, arc, dwell and reviewed transition waypoint. Include operation/path boundaries so empty or skipped paths cannot pass unnoticed. Tokens identify uses, not just shared geometry records or endpoint coordinates.

As the emitter processes a use, associate its command/map entry with that token. Before returning a translation, require every expected movement/dwell use exactly once and in source order, with no unrecognized uses, balanced boundaries and a completed program end. Repeated points, reversals and repeated/shared paths are distinct required uses. All checks run automatically in preflight and translation, with whole-program buffering retained.

Do not require one G-code line per process-state request: unchanged feed/spindle/coolant requests can legitimately suppress lines. Instead check that the applicable semantic request was considered and the emitted state at the affected motion agrees with the current contract, including the decoded-to-output CSS unit conversion. Preserve explicit unknown-state limitations after M6. No controller feedback or physical offset transformation is inferred.

Build expected coverage from the validated input, not from emitted lines; otherwise an omitted move could disappear from both sides of the audit. The current `test/support/canonical.js` association helper is a starting specification, not a production dependency. Keep independently authored expected fixtures and the native interpreter checks as separate evidence against shared implementation errors.

Also check one source-map entry per emitted line, consecutive line numbers, correct operation ranges, and agreement between command fields and motion details. A mismatch is an internal error carrying the first missing/duplicate/misordered use and its source. Emit a compact completeness summary in the existing diagnostic report; no user-selectable audit or error-skipping mode.

Candidate files: a small `src/execution-audit.js`, `src/translate.js`, report/archive support and focused audit tests. Use iterator/counter state where possible so the audit does not introduce an unnecessary second full copy of every motion.

Acceptance: fault injection that drops, duplicates, reorders or misattributes a move/dwell/plan waypoint fails for the intended reason before any output is published. Include identical repeated endpoints, reused geometry, full circles and continue/link/retract boundaries for both machines. The full existing Node and standalone LinuxCNC suites remain required; retain exact-number tests because the native canonical trace rounds coordinates.

## Scope and delivery

Implement these as three reviewable changes in order. Step 1 supplies useful source context for steps 2 and 3; internal command validation can be developed independently. Each change must retain accepted lathe/mill profiles, execution-plan schemas 1–4, public translation API and exact baseline G-code apart from an intentional release banner. A Node engine requirement or runtime dependency change is unnecessary.

Fusion continues to own toolpath geometry, compensation and machining strategy. The execution plan supplies reviewed mappings and transitions; LinuxCNC owns actual tool/WCS values, motion control and machine configuration. No new clearance, feed, tolerance, rounding, axis-suppression or controller-style options belong in this work.

Do not import STEPNode's global Finder singleton, silent argument-return macro, demo output streaming, arbitrary demo moves or formatted-number equality. Keep per-translation state, typed errors, complete validation and exact serialized coordinates. The public test tree is mainly test scaffolding/documentation and is not an independent lathe/mill conformance suite. [S4, S5, S6]

Defer STEPNode installation, SDK integration, stock simulation, selective execution, run-from-workingstep/restart, feature generation and general AP238 import. Its arc-height API alone does not establish a compatible helix representation for our profiles. The earlier [native-helix research proposal](nist-toolkit-review-plan.md#4-optional-research-preserve-native-milling-helices) remains separate.

A later SDK interoperability experiment would first need an obtainable supported SDK, clear terms and a pinned isolated environment. Test synthetic files in both directions and compare decoded semantics against independently specified expectations; do not use the Okuma demo as a LinuxCNC oracle. This is optional research, not a prerequisite for the three changes above.

## Pinned sources

- **S1:** [Archived project README](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/README.md), [repository license](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/LICENSE).
- **S2:** [Build dependencies](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/BUILDING.md), [native libraries](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/binding.gyp).
- **S3:** [Cursor events](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/src/nodeAdaptive.cpp#L34), [execution selection and active objects](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/src/nodeAdaptive.h#L59), [position/source metadata](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/src/nodeAdaptive.cpp#L1167), [arc metadata](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/src/nodeAdaptive.cpp#L1340).
- **S4:** [Formatter delegates to SDK](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/src/nodeGenerate.cpp#L174), [state API](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/src/nodeGenerateState.h#L145), [rounded-number comparison API](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/src/nodeGenerate.cpp#L668).
- **S5:** [Demonstration translator](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/examples/generate.js#L47).
- **S6:** [Finder singleton and argument handling](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/src/nodeFinder.cpp#L26), [test documentation](https://github.com/steptools/STEPNode/blob/220fe390ff7c316d5a304e1485a870758ae06e69/test/README.md), [pinned test tree](https://github.com/steptools/STEPNode/tree/220fe390ff7c316d5a304e1485a870758ae06e69/test).
- **T1:** [Existing internal command formatter](https://github.com/markmounteer/LinuxCNCNext-NC/blob/2f2c1436654a1f808fbeea329e6628aa944ce0d8/src/linuxcnc-output.js), [state contract and M6 limits](https://github.com/markmounteer/LinuxCNCNext-NC/blob/2f2c1436654a1f808fbeea329e6628aa944ce0d8/docs/command-state.md).
- **T2:** [Shared inspector and fingerprint construction](https://github.com/markmounteer/Fusion360Next-NC/blob/cfda07768f09554148db7b2049d3ad608149c91e/lib/inspect.js#L224), [parser locations](https://github.com/markmounteer/Fusion360Next-NC/blob/cfda07768f09554148db7b2049d3ad608149c91e/lib/part21.js#L71).
- **T3:** [Current translation traversal and maps](https://github.com/markmounteer/LinuxCNCNext-NC/blob/2f2c1436654a1f808fbeea329e6628aa944ce0d8/src/translate.js), [preflight and archive contract](https://github.com/markmounteer/LinuxCNCNext-NC/blob/2f2c1436654a1f808fbeea329e6628aa944ce0d8/docs/preflight.md).
- **T4:** [Existing association checks](https://github.com/markmounteer/LinuxCNCNext-NC/blob/2f2c1436654a1f808fbeea329e6628aa944ce0d8/test/support/canonical.js), [current semantic regressions](https://github.com/markmounteer/LinuxCNCNext-NC/blob/2f2c1436654a1f808fbeea329e6628aa944ce0d8/test/semantics.test.js).
