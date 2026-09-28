# STEPNCpp: review and improvement plan

Status: steps 1–3 implemented for v0.8.0 on 2026-09-28. See [process summaries and job requirements](process-summary.md) for the delivered report contract and [validation](validation.md) for current test evidence. The pinned review and original plan follow; the deferred feature-cycle, tolerance and feedback research remains outside this release.

## Recommendation

Use [johnmichaloski/STEPNCpp](https://github.com/johnmichaloski/STEPNCpp), fifth in the earlier **execution-side** ranking, as a reference for explaining how source machining parameters become controller instructions. Its Siemens feature-cycle implementation is not a suitable LinuxCNC backend to import.

Build on v0.7.0 with three bounded improvements, in this order:

1. A complete per-operation process summary with explicit source and output units.
2. An automatically derived job-requirements summary that distinguishes translation checks from unverified controller capabilities.
3. Systematic geometry and unit-equivalence regressions for XZ lathes and fixed-axis XYZ mills.

These improve diagnosis and confidence in translation. They do not promise faster machining or shorter G-code. They require no new Fusion machining options, alternate tool tables, machine-limit settings or controller-style selectors.

## Verified baseline and limits

- Upstream `master`: `067c9a774c997670a33066ebb311d5ce152fd931`, dated 2021-06-25; tree `30157cf4291db82647db9299872afbb058841be3`. GitHub does not mark it archived. Its README describes a historical collection of work from 2008; the 2021 repository date is not evidence of contemporary controller validation. [S1]
- Target: LinuxCNCNext-NC v0.7.0, clean local and remote `main` at `b0592dbbbee1e3ef25976c5aa373f0e7ed3c1521`. Fusion360Next-NC was inspected at `a6e64622343c41545925ebf6e8dc91e12d320ac1`.
- Downloaded 38 selected upstream files, 623,366 bytes, from the pinned tree and checked their bytes against Git blob hashes. The manifest records Git SHA-1 and SHA-256 for each file. Reviewed relevant source sections in the Siemens mappings, cycle parameter/verification code, tooling, regression scaffolding, project configuration and README. This was not a review of every file in the repository.
- Local source snapshot and manifest: `D:\dev\Next-Nc research\github-translators-2026-09-28\evidence\johnmichaloski--STEPNCpp\detailed-review\`.
- Reran the target Node suite: **35 passed, one POSIX-only test skipped on Windows, zero failed**. Also translated the existing synthetic semantic fixture for each machine and unit system and compared the operation summary with the detailed command map; results are saved as `target-report-probe.json` beside the local source manifest.
- STEPNCpp was **not built or run**. Its historical simulations and cutting experiments were not independently reproduced. Native LinuxCNC tests were not rerun for this documentation-only review; existing v0.7.0 interpreter evidence is in [validation.md](validation.md).
- No private job files were published, and no LinuxCNC installation, configuration or physical machine was changed.

## What the source actually offers

The useful structure is a mapping from named source parameters, through derived values, to controller output. `CannedCyles.h` distinguishes parameter origins such as feature, operation and workpiece, and categories such as length, feed and speed. `FeatureCycles.cpp` includes parameter/error report generation. That structure can make an existing translation easier to inspect without introducing feature generation. [S2]

The reviewed mappings generate Siemens 840D instructions such as `CYCLE71` and `CYCLE832`. They derive machining decisions from features; our bounded profiles already contain Fusion's explicit toolpaths. Replacing those paths with feature cycles would require information and semantics that the current profiles do not supply. The reviewed code does not establish a working XZ-lathe LinuxCNC backend. A bundled turning/tool resource schema and generic XZ-plane enum do not establish one. [S3, S4]

Several source details argue against direct adoption:

| Finding | Consequence for this project |
| --- | --- |
| In `NiStep/Mappings/Siemens/Seimens840D.cpp`, the generation-loop verification call is commented out. `IWorkCycle::Verify` also has its substantive check disabled. The separate older `StepNCAnnotator` generator calls verification but logs failure and proceeds to output. [S3, S5] | Retain our complete validation before publication. Do not describe upstream verification as a proven safeguard. |
| The facing implementation explicitly records untested bidirectional and second/third/fourth-quadrant cases. [S4] | Use this as motivation for systematic coordinate-transform tests, not as a motion oracle. |
| Tool lookup replaces nonpositive length with `100.0`; spindle output casts speed to an integer; several output fields use three decimals. [S3] | Preserve missing-value rejection, exact numeric output and independent T/H mappings. Never invent tool dimensions or silently truncate process values. |
| Facing converts feed with `*1000*60`; pocket mapping supplies hard-coded roughing/finishing tolerances and Siemens-specific cycle commands. [S4] | Unit conversions must follow our profile's actual quantities. Do not transplant constants or reinterpret CAM tolerance as a LinuxCNC blending allowance. |
| The reviewed regression entry points leave `TestAll` commented out. The project uses Visual C++ 9, ATL/WTL/COM-era dependencies and machine-specific build paths. [S6] | There is no reproduced, portable lathe/mill conformance result to inherit. Keep the current dependency-free Node implementation and independent LinuxCNC tests. |

GitHub identifies no repository-wide license. Some source files carry U.S. government notices, while the tree contains third-party material. Those notices do not establish uniform redistribution terms. Implement the ideas independently; do not copy this source into the MIT project or add its binaries as dependencies. [S1, S2, S6]

## What v0.7.0 already does

The translator already has typed internal commands, per-translation modal state, STEP-record provenance, an ordered completeness audit, exact serialization, aggregate execution-plan checks, optional tool-table snapshot validation, atomic output, local diagnostics and HTML reports. Native tests already cover both machines, both units, principal arc planes, nonzero offsets, process changes and deliberate corruptions. Those are not new proposals from this review. [T1, T2]

The remaining reporting gap is concrete. In the existing `semanticFixture`, operation **Link and reverse** starts with flood coolant and a clockwise spindle, then changes to 700 RPM counterclockwise, mist coolant and feed per revolution. The detailed source map records those changes correctly. The operation summary still presents the initial spindle/coolant object, and generic numeric fields such as `speed` require the reader to infer their units. On the lathe, the initial decoded CSS value is `80000` mm/min while the emitted LinuxCNC value is `S80` m/min. This is a presentation limitation, not a demonstrated motion error. [T3, T4]

## 1. Explain every process phase and quantity

Add a small, pure report-building module, tentatively `src/process-summary.js`. Consume the validated decoded program, existing provenance and audited output map; do not reparse STEP or create another translation engine.

For each operation, show initial process state and ordered changes throughout its paths: spindle mode/value/direction/cap, coolant, feed mode/value and dwells. Connect rows to the existing STEP properties, path ordinals and candidate G-code line numbers/hash. Show retained modal values as inherited, rather than pretending an extra command was emitted. Distinguish source-requested settings from translator-generated stops, initialization and transition commands. Preserve the current unknown-state treatment around M6.

Give numeric rows an explicit quantity kind, decoded source value/unit, output value/unit and conversion explanation. For example:

| Quantity | Decoded source | LinuxCNC meaning |
| --- | --- | --- |
| Metric lathe CSS | 80,000 mm/min | `G96 S80`, 80 m/min |
| Inch lathe CSS, separate example | 1,200 in/min | `G96 S100`, 100 ft/min |
| Constant spindle speed | 700 RPM, counterclockwise | `G97 S700` and `M4` |
| Feed per revolution | 0.2 mm/rev | `G95 F0.2` |
| Dwell | 0.25 seconds | `G4 P0.25` |

LinuxCNC's CSS units differ from its linear coordinate units; feed-per-minute and feed-per-revolution must also remain distinct. [L1] Keep spindle direction separate from its nonnegative decoded magnitude. Label lathe X as radius. A mode-only command without a new numeric value must not acquire a fabricated zero. Do not round the values used for comparisons or generation.

Add this as optional structured report data with its own version marker, outside the fingerprinted program model. Render readable tables in `src/report.js`, retain the detailed source map, escape all source text, and make older archives say that a summary was not recorded. Do not silently reconstruct missing original-source quantities from output alone. The source-map schema and existing public fields remain compatible.

**Acceptance:** hand-specified report expectations for both machines and units, CSS-to-RPM changes, both spindle directions, coolant changes, G94/G95, dwell, inherited state and repeated values. Every phase must resolve to its existing source/output context. Old and hostile-text archives must still render safely. G-code and fingerprints remain byte-for-byte unchanged, apart from a separately intentional release banner.

Candidate integration points: `src/translate.js`, `src/report.js`, `test/review.test.js` and a focused process-summary test file. No CPS or shared-reader change is required for the current decoded quantities.

## 2. Derive job requirements automatically

Use all validated paths and plan transitions, not just each operation's initial state, to summarize what the job requests. The existing source map supplies actual output evidence; the decoded model supplies source quantities.

Include machine/axes, program units and lathe radius convention, arc planes, spindle 0 modes and directions, CSS caps where applicable, feed modes and dimensioned ranges, coolant commands, mapped T/H/WCS records, and counts of reviewed continue/link/retract boundaries. Separate RPM from CSS ranges and exclude translator-generated shutdown `S0` from requested cutting-speed ranges. CSS plus a cap is not an estimate of the actual spindle-speed history.

Use clear evidence categories:

- **Translation validated:** supported semantics, command contract and source/output completeness passed.
- **Table snapshot checked / not checked:** reuse the existing tool-table result; this establishes only the documented snapshot checks.
- **Controller commissioning not checked:** actual spindle feedback, M6/remaps, coolant wiring, calibrated tool/WCS values, travel and physical clearance.

For example, if any path uses G95, identify spindle-speed feedback as a controller requirement. A later G95 path must not disappear because an operation started in G94. For CSS, identify the existing X0/offset prerequisite. These statements explain requirements; they must not claim that offline translation verified HAL wiring or machine setup. [L1]

This is automatically generated report content, not a new machine-capability configuration file. LinuxCNC remains authoritative for its settings. Keep `doctor` scoped to installation checks; preflight/translation archives receive the job summary. Preserve current failures for unsupported profiles or incomplete required values, and avoid duplicating those validators.

**Acceptance:** both-machine fixtures whose later paths introduce different feed/spindle/coolant settings; with/without tool-table cases; separate CSS/RPM and G94/G95 ranges; explicit unverified controller status; consistent JSON/HTML results. No extra user options, files to maintain, or changes to accepted profiles and plans.

## 3. Systematically test coordinate and unit invariants

Existing tests already contain negative coordinates, off-origin geometry and all milling arc planes. Extend them with a small deterministic matrix whose expected relationships are specified independently:

- Translate synthetic work-coordinate paths and reviewed work-coordinate approach/link points into all sign quadrants. Arc endpoints and centres shift together; relative I/J/K and arc sense remain unchanged. Machine-coordinate G53 retract points do not receive the work-coordinate translation.
- For XZ lathes, exercise radius coordinates, both directions, major arcs crossing the angular wrap, full circles and repeated/reversed vertices. For XYZ mills, add the same cases in XY, XZ and YZ with independently specified principal-plane orientation. These remain fixed-tool-axis cases, not rotary machining.
- Create physically equivalent metric/inch pairs, scaling geometry, feed and the appropriate plan/tool/WCS test data. RPM, spindle direction and dwell remain unchanged. One easy CSS pair is 30,480 mm/min and 1,200 in/min, producing 30.48 m/min and 100 ft/min. Unit-equivalent files have their own fingerprints and reviewed plans; never bypass plan binding.
- Exercise tolerated curve joins and full circles using the actual commanded starting position, preserving the existing exact-coordinate/axis-omission behavior. Keep work and machine frames separate throughout the comparison.

Do not use the production conversion or arc helpers to generate their own expected answers. Retain hand-authored oracles and fault injection. Run the representative new cases through standalone LinuxCNC as well as Node. Native trace comparisons retain the documented precision allowance; they do not replace exact-number Node tests or justify rounding output.

**Acceptance:** every matrix cell is named and exercised; deliberate wrong scale, centre sign, plane, arc direction or G53/work-frame treatment is caught. Existing golden G-code, fingerprint, source-provenance, completeness, CLI and no-partial-output tests all continue to pass. Add only missing coverage, not a second general fuzzing framework.

Candidate files: `test/support/` fixtures, `test/semantics.test.js`, a focused geometry-invariants test, and `scripts/test-linuxcnc-semantics.js`.

## Delivery and deferred work

Deliver steps 1–3 as reviewable commits, maintaining both lathe and mill coverage in each. Steps 1 and 2 share one report representation; step 3 can proceed independently. Run the current Windows/Linux Node matrix and standalone LinuxCNC suite before releasing behavior changes. Preserve profile versions, execution-plan schemas 1–4, exact output, fingerprints and the public translation API. Update report/validation documentation with actual evidence, rather than treating this plan as implemented.

Fusion continues to own machining strategy, compensation and generated geometry. The reviewed plan supplies explicit mappings/transitions. LinuxCNC owns real machine state, offsets, motion planning and feedback. The immediate work is in the translator and tests; no new post-processor properties are needed.

Defer these larger ideas:

- **Feature-derived canned cycles:** require a versioned feature/operation contract and independent expansion equivalence. Do not infer a cycle from a toolpath shape or operation name. Siemens cycles are not LinuxCNC drop-ins.
- **Tolerance-driven blending:** retain G61. Any future G64 policy needs an explicit remaining error budget and validated geometry/finish consequences; the current profile provides no such budget. Upstream hard-coded rough/finish values are not suitable defaults.
- **Tool/stock geometry validation:** tool numbers and H records are not cutter/holder geometry. A future producer contract must distinguish missing dimensions, cutting-edge length, assembly length and calibrated offsets. Do not invent defaults or claim collision checking from the current model.
- **Execution feedback:** source-to-line provenance is useful groundwork, but planned commands and actual execution must remain distinct. A future read-only controller adapter would need exact job identity, units, timestamps and restart handling before joining feedback to operations. No adaptive feed changes, controller control or run-from-line functionality follows from this review.

## Pinned sources

- **S1:** [Repository README](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/Readme.md), [pinned tree](https://github.com/johnmichaloski/STEPNCpp/tree/067c9a774c997670a33066ebb311d5ce152fd931).
- **S2:** [Parameter origins and types](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/StepAnnotator/CannedCyles.h#L21), [cycle verification and reports](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/StepAnnotator/FeatureCycles.cpp#L62), [expression-based verification](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/StepAnnotator/CannedCyles.cpp#L602).
- **S3:** [Siemens tool defaults and output](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/Mappings/Siemens/Seimens840D.cpp#L103), [disabled generation-loop verification](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/Mappings/Siemens/Seimens840D.cpp#L245), [feature dispatch](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/Mappings/Siemens/Seimens840D.cpp#L169).
- **S4:** [Facing limitations and mapping](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/Mappings/Siemens/SiemensFacing.h#L10), [pocket tolerance/cycle mapping](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/Mappings/Siemens/SiemensRectangularPocket.h), [tool parameter distinctions](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/StepAnnotator/StepNCTooling.h).
- **S5:** [Older generator's log-and-continue behavior](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/StepNCAnnotator/StepNCAnnotator/Seimens840D.cpp#L161).
- **S6:** [Regression entry points](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/StepNCAnnotator/StepNCAnnotator/Tests/UnitTests.cpp), [Visual C++ project](https://github.com/johnmichaloski/STEPNCpp/blob/067c9a774c997670a33066ebb311d5ce152fd931/NiStep/StepAnnotator/StepAnnotator.vcproj).
- **T1:** [Current command contract](https://github.com/markmounteer/LinuxCNCNext-NC/blob/b0592dbbbee1e3ef25976c5aa373f0e7ed3c1521/src/command-contract.js), [ordered completeness audit](https://github.com/markmounteer/LinuxCNCNext-NC/blob/b0592dbbbee1e3ef25976c5aa373f0e7ed3c1521/src/execution-audit.js).
- **T2:** [Current validation evidence](https://github.com/markmounteer/LinuxCNCNext-NC/blob/b0592dbbbee1e3ef25976c5aa373f0e7ed3c1521/docs/validation.md).
- **T3:** [Current operation summary rendering](https://github.com/markmounteer/LinuxCNCNext-NC/blob/b0592dbbbee1e3ef25976c5aa373f0e7ed3c1521/src/report.js#L20), [source/output process conversion](https://github.com/markmounteer/LinuxCNCNext-NC/blob/b0592dbbbee1e3ef25976c5aa373f0e7ed3c1521/src/translate.js#L35).
- **T4:** [Synthetic fixture and independent expected G-code](https://github.com/markmounteer/LinuxCNCNext-NC/blob/b0592dbbbee1e3ef25976c5aa373f0e7ed3c1521/test/support/semantic-fixture.js).
- **L1:** LinuxCNC 2.9 documentation: [G94/G95 feed modes](https://linuxcnc.org/docs/2.9/html/gcode/g-code.html#gcode:g93-g94-g95), [G96/G97 spindle modes and units](https://linuxcnc.org/docs/2.9/html/gcode/g-code.html#gcode:g96-g97), accessed 2026-09-28.
