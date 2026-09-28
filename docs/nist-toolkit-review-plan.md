# NIST ISO 14649 toolkit: review and improvement plan

Status: steps 1–3 implemented for v0.6.0 on 2026-09-28 UTC; independent native CI verification pending. Step 4 remains optional future work. The original review and its pinned v0.5.0 baseline follow for provenance. See command-state.md and validation.md for implementation details.

## Recommendation

Use [mhaberler/iso-14649-toolkit](https://github.com/mhaberler/iso-14649-toolkit) as a reference for explicit interpreter state, canonical command semantics and diagnostic propagation. Retain the current Node translator and its Next-NC/AP238 input contract. The toolkit is the **third execution-side repository** in the earlier ranking.

The immediate benefits are better explanations of geometry failures and stronger evidence that generated LinuxCNC commands preserve the intended state and geometry. These improvements do not require new machining options, another CAM planner, or a native STEP-NC controller.

### Review baseline

- Upstream default-branch HEAD, verified through GitHub: `0e3c10eff65c4b22e06f2e5ce2faeabe287348c8`, committed 2009-02-01; tree `78d1fdadebdd4c4197d506366d80d8f0733608f9`. The repository is not marked archived, but the age of this source should not be confused with active maintenance.
- Target local and GitHub HEAD: `abf37283db57a56aa62cde0370c99e1ef7b90329`, LinuxCNCNext-NC v0.5.0. The checkout was clean before this document.
- Reviewed the upstream README, interpreter, canonical implementation/header, driver, state/error header, Makefile and bundled feature/explicit-path examples. Cached source bytes were verified against the pinned Git blob hashes; additional header/sample downloads were similarly verified.
- Reran the target Node suite: 25 passed, one POSIX-only test skipped on Windows, zero failures. Reproduced the diagnostic gap below using the synthetic turning fixture in memory.
- The upstream toolkit was **not compiled or executed**. Its bundled canonical outputs are reference files, not a fresh successful test. No controller or physical machine was used.

Research snapshots and manifests are under `D:\dev\Next-Nc research\github-translators-2026-09-28\evidence\mhaberler--iso-14649-toolkit\`; additional files are in its `nist-review` directory.

## What the repository provides

The toolkit contains generated C++ data classes, a YACC/Lex Part 21 parser, parser/printer applications and a limited three-axis machining interpreter. Its interpreter emits canonical machining calls. The supplied `canon.cc` prints those calls and maintains a dummy external-world model; it is not a ready LinuxCNC G-code backend. [N1, N2]

The supplied explicit-path example declares `MACHINING_SCHEMA` and `MILLING_SCHEMA`. Our producer instead emits `INTEGRATED_CNC_SCHEMA` with explicit Next-NC profile properties. Renaming extensions or headers cannot make the two data models compatible. Upstream's machining support is milling-oriented, with no reviewed lathe/CSS contract matching ours. [N1, N7, T1]

An important distinction: the canonical interface describes XY, YZ and ZX arcs, but the interpreter's `follow_circle_forward` and `follow_helix_forward` require a vertical axis and write XY-style arc arguments. `run_code` also updates arc position with an XY layout. Consequently, its interpreter is not an independent oracle for our XZ turning or all-plane milling paths. Use the interface's coordinate definitions and LinuxCNC's interpreter for those checks. [N2, N3]

The source carries a NIST notice describing its U.S. government copyright status; the README says no license is required. GitHub's detected license is null. Prefer independently authored Node changes and synthetic fixtures; if any upstream code or samples are later redistributed, retain the applicable file notices and provenance rather than relabeling them as original MIT code. [N1, N2]

## Findings mapped to this project

| Upstream evidence | Current target evidence | Planned use |
| --- | --- | --- |
| Named error conditions propagate through a bounded function-context stack. [N4] | Some reader checks include record/line context, but many geometry checks use a generic exception. A changed synthetic circle radius produces `INVALID_NEXTNC`, `arc endpoint radius mismatch`, and empty context. [T2] | Extend existing diagnostics with rule, operation, path, entity and relevant numeric values. |
| Geometry and process decisions are separated from canonical command calls; the world model tracks modal state. [N2, N4] | `translate.js` combines state tracking, G-code formatting and source-map creation. Motion entries are detailed; a `G95 F0.1` line is represented only as a generic state action with operation/path context. [T3] | Introduce a small internal structured command layer and record semantic state changes. |
| `ARC_FEED` defines axis order and signed rotation independently of text formatting. [N2] | Existing tests already cover all three planes, both senses, full circles and mm/inch. Native tests check selected canonical arrays and state counts. [T4] | Extend comparisons to complete ordered motion/process events and boundary state; do not claim basic plane tests are missing. |
| Feature-based and explicit-path sample programs have identical bundled `.can` outputs. [N7] | Current fixtures test known paths, links and continuations; they do not provide a general state-aware semantic comparison harness. [T4] | Adopt paired-representation tests with independently specified expected behavior. Do not use rounded `.can` text as a precision oracle. |
| The interpreter has default security planes, approach distances, tool changes and feature generators. [N3, N4] | Fusion supplies paths/process settings; reviewed plans supply missing machine transitions; LinuxCNC owns offsets and execution. [T1] | Keep these responsibilities. Do not import upstream clearance constants, feed calculations or strategy generation. |

## Implementation sequence

### 1. Preserve geometry-error provenance — first deliverable

Extend the shared inspector's existing `recordCheck` pattern so failures retain the owning operation and path as well as the responsible STEP entity. Use the parser's `doc.locations` map; do not rescan the file or infer a line from the error text.

For an arc-radius failure, report the operation/section, path, circle and trimmed-curve records, source line, plane, units, start/end/center, both measured radii, mismatch and the acceptance threshold actually used. For continuity failures, report previous commanded exit, next source start and the comparison threshold. Include a stable rule identifier and validation stage. Context that is not yet known must remain absent, not fabricated.

Preserve public top-level error codes such as `INVALID_NEXTNC` and the existing first-error/aggregate-plan contracts. Add structured context rather than breaking consumers or changing tolerances. Catch and enrich expected validation failures; do not disguise unexpected programming errors as ordinary malformed geometry. The HTML report should display actionable context and correction ownership: regenerate/review the Fusion operation for source geometry, edit the named execution-plan field for a plan failure, inspect the existing table snapshot for a mapping failure.

Files: upstream `Fusion360Next-NC/lib/inspect.js` and its reader tests; then a pinned vendor update in this repository, `src/profile.js`, `src/errors.js` if needed, `src/report.js`, diagnostic/CLI regression tests and `vendor/fusion360next-nc/PROVENANCE.md`. Avoid a permanently divergent vendored inspector.

Acceptance: malformed lathe and mill fixtures identify the actual source record and operation; shared geometry correctly identifies the failing use; the original error code remains stable; preflight and translate produce the same failure details and no G-code; older archives still render. Successful models, fingerprints and G-code remain unchanged.

### 2. Record explicit commands and modal state

Refactor the existing emission boundary incrementally. Keep the decoded geometry model, public `translate(text, plan, options)` API and reviewed execution plans. Add an internal command object for each semantic emission: mode/plane selection, tool selection, H/WCS application, spindle speed/mode/direction, coolant, feed mode/value, rapid, linear, arc, dwell and program end. Every record carries source operation/path/phase and its coordinate frame.

Use named fields, not the toolkit's seven-number arrays. Keep XYZ positions explicit; translate to G17/G18/G19 word order only in the LinuxCNC formatter. Distinguish commanded position from reconstructed source geometry and unknown machine position. A G53 waypoint supplies a machine-coordinate target for one axis; it must not silently become a known work-coordinate XYZ position.

A per-translation state object should govern redundant-word suppression and modal resets. Preserve the exact existing M6 sequence and reassertion, lathe CSS/G95 behavior, independent T/H mappings, and continued/link boundaries. No global interpreter singleton or live `GET_EXTERNAL_*` calls are needed in an offline translator.

The existing source map can gain additive semantic command/state fields and before/after values where relevant. Build G-code and its map from the same command records. Keep the objects internal for this release: no new exchange language, public command-file input, new controller backend or machine-settings API.

Candidate files: `src/translate.js`, a small `src/commands.js` or `src/linuxcnc-output.js`, `src/report.js`, `test/translation.test.js`, `test/milling.test.js`, `test/preflight.test.js` and a focused state test module.

Acceptance: byte-for-byte G-code equivalence for the baseline fixture corpus, excluding only an intentional release-banner change; unchanged decoded fingerprints and accepted plans; every emitted motion/state transition has a consistent source entry; separate calls cannot leak tool, feed, plane or spindle state. Existing output buffering and exclusive file writes remain intact. A formatter failure must still leave stdout/output empty.

### 3. Strengthen independent semantic verification

Expand the existing standalone LinuxCNC `rs274` tests. Compare ordered interpreted motions and process-state changes, using the current interpreter's actual canonical output format. Normalize plane-dependent coordinates explicitly: G17 maps X/Y with Z perpendicular, G18 maps Z/X with Y perpendicular, and G19 maps Y/Z with X perpendicular. Match arcs by center, endpoint, sense and turns; validate feed mode/value, spindle mode/direction/cap, coolant and dwell at the affected motion.

Do not use only the newly generated command list as the expected answer: it could share the emitter's bug. Use independently authored expected vectors/state transitions for small fixtures, plus decoded-input comparisons and the LinuxCNC trace as separate evidence. Canonical text may round values; record its precision and comparison bounds, and retain exact-number Node tests for values the trace cannot resolve.

Prioritize these additional cases:

- Deliberately different, nonzero synthetic T/H offsets and nonzero work offsets. Current native fixtures use zero-offset records, so merely selecting H3 instead of H2 does not prove their physical offset distinction. Assert canonical offset changes and motion in the interpreter's documented frame; keep tool-table values in machine units.
- Plane changes across `continue`, `link` and `retract` boundaries, including reassertion after a tool change; feed changes after a rapid/dwell; spindle reversal and coolant changes next to those boundaries.
- Off-origin arcs, major arcs crossing angular wrap, exact full circles, invalid normals and radius/join values immediately around existing validation thresholds, in mm and inches.
- Paired explicit waypoints/compacted polylines and shared/unshared geometry with the same intended execution. Preserve repeated and reversed moves; do not treat matching endpoints as equivalent paths.
- Deliberately corrupt one generated plane, I/J/K center, G94/G95 mode, spindle sign, H record or source-map association and demonstrate that the verification detects it.

Files: `scripts/test-linuxcnc.js`, focused test helpers and synthetic fixtures, relevant Node tests, `.github/workflows/ci.yml`, `docs/validation.md`. Add failure artifacts containing the exact program and interpreter trace so a CI discrepancy is reproducible. No private job files are needed.

Acceptance: both lathe and XYZ mill suites pass in mm/inch, and each deliberate corruption fails for the intended reason. The result remains an offline interpreter check, not proof of clearance, feedback wiring, M6 remaps or physical machining.

### 4. Optional research: preserve native milling helices

NIST's canonical interface shows how an arc can include perpendicular-axis travel and a signed number of turns. This could inform a later coordinated post/translator extension that preserves Fusion's analytic milling helices instead of expanding them into many linear moves. [N2, N3]

This is **not** part of the first implementation batch. Our current milling profile accepts planar arcs, and the CPS already linearizes helices. A consumer cannot reliably recover original helix intent from those polylines. The ISO 14649 `helix` entity is not interchangeable with our AP238 graph; first establish an appropriate AP238 representation, its units/sweep/direction semantics and a new explicit profile version. A similarly named schema entity alone is insufficient evidence of compatibility.

Only proceed after independent parser/geometry checks and actual Autodesk callback tests demonstrate endpoints, center, plane, axial travel and turn count. Test CW/CCW, both axial directions, full/partial/multiple turns, mm/inch and LinuxCNC G2/G3/P behavior. Older consumers must reject the new profile rather than dropping axial travel. Keep existing lathe files and planar mill files compatible.

Do not fit helices automatically to existing paths, copy the toolkit's pitch-to-turn rounding rule without validation, enable threading/tapping through this feature, or promise a shorter machining time merely from fewer lines. Retain the current tolerance-controlled Fusion linearization until the new contract is established.

## Boundaries and delivery

Implement steps 1–3 as separate reviewable changes, starting with diagnostic provenance. They require no change to the accepted machining profiles or user-defined post properties. Step 1 touches the shared Fusion inspector, but does not require a CPS behavior change; native helices would.

Keep the existing v0.5.0 HTML reports, doctor, aggregate plan validation, tool-table checks, XYZ schema 4 and optional AXIS helper. Those are already implemented and are not deliverables to repeat.

Do not adopt NIST's driver option to continue after errors, its immediate printing behavior, four-decimal formatting, fixed `retracted_z = 250`, `ISO14649_UP = 5`, broad `ISO14649_TINY = 0.0001`, feature-based generators or machine synchronization. Preserve our whole-program validation, exact serialized coordinates, explicit reviewed transitions and controller-owned settings. [N2, N3, N4, N5]

An optional future pinned Linux build of the NIST toolkit can reproduce its own examples in isolation. That would establish only that its supported ISO 14649 examples run; it would not validate our AP238 profile, lathe behavior or the user's machine. It is not a dependency or prerequisite for the proposed Node improvements.

## Pinned source references

- **N1:** [Upstream README](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/README): components, limited implemented interpreter and build instructions.
- **N2:** [Canonical interface and arc contract](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/include/canon.hh#L229), [dummy backend and formatting](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/source/canon.cc#L272).
- **N3:** [Circle restriction](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/source/iso14649interp.cc#L5347), [helix handling](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/source/iso14649interp.cc#L5533), [command dispatch/position update](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/source/iso14649interp.cc#L10396), [initialization defaults](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/source/iso14649interp.cc#L11001).
- **N4:** [Error propagation, tolerances and state model](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/include/iso14649interp.hh#L49).
- **N5:** [Driver error/recovery policy](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/source/driver.cc#L254).
- **N6:** [Makefile](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/Makefile).
- **N7:** [Explicit-path example](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/data/ex1E.stp#L23), [its bundled canonical output](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/data/ex1E.can), [feature-example output](https://github.com/mhaberler/iso-14649-toolkit/blob/0e3c10eff65c4b22e06f2e5ce2faeabe287348c8/iso-14649-toolkit/data/ex1.can). Both output files have SHA-256 `6422efe417222e72743fb3588aabffb9eb81e8b3e7c095bc46c657e72d77e0de`.
- **T1:** [Current machine/profile boundary](https://github.com/markmounteer/LinuxCNCNext-NC/blob/abf37283db57a56aa62cde0370c99e1ef7b90329/docs/milling.md), [plan validation](https://github.com/markmounteer/LinuxCNCNext-NC/blob/abf37283db57a56aa62cde0370c99e1ef7b90329/src/plan.js).
- **T2:** [Current geometry inspector](https://github.com/markmounteer/LinuxCNCNext-NC/blob/abf37283db57a56aa62cde0370c99e1ef7b90329/vendor/fusion360next-nc/inspect.js), [consumer error wrapping](https://github.com/markmounteer/LinuxCNCNext-NC/blob/abf37283db57a56aa62cde0370c99e1ef7b90329/src/profile.js).
- **T3:** [Current emission/state/source mapping](https://github.com/markmounteer/LinuxCNCNext-NC/blob/abf37283db57a56aa62cde0370c99e1ef7b90329/src/translate.js).
- **T4:** [Current LinuxCNC interpreter checks](https://github.com/markmounteer/LinuxCNCNext-NC/blob/abf37283db57a56aa62cde0370c99e1ef7b90329/scripts/test-linuxcnc.js), [milling regressions](https://github.com/markmounteer/LinuxCNCNext-NC/blob/abf37283db57a56aa62cde0370c99e1ef7b90329/test/milling.test.js).
