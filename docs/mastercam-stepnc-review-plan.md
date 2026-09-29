# mastercam-stepnc: review and improvement plan

Status: reviewed on 2026-09-28 against LinuxCNCNext-NC v0.10.0; the three-step
plan below is **implemented in v0.11.0**. The review findings and experiments
below describe the pinned v0.10.0 baseline. See
[policy verification](policy-verification.md) for the implementation and
[validation](validation.md) for test evidence. Research probes remain local.

## Recommendation

Use [steptools/mastercam-stepnc](https://github.com/steptools/mastercam-stepnc)
as a reference for preserving CAM process state across tool changes and
operation boundaries. Its explicit distinction between toolpath start, modal
state, coordinate orientation and compensation is useful. It is a Mastercam
exporter, not a STEP-NC reader or LinuxCNC backend. [S1, S2]

Prioritize three improvements for **XZ lathes and fixed-axis XYZ mills**, both
mm and inch:

1. Explicitly reestablish the translator's stopped-spindle/coolant-off policy
   after an actual M6, before the reviewed work-coordinate approach.
2. Independently audit required process-control commands and their order at
   startup, retract/tool-change boundaries, reversals and shutdown.
3. Add focused state-transition and fault-injection regressions, with truthful
   policy-audit coverage in JSON/HTML and native interpreter evidence.

The target already has extensive arc, unit, toolpath-completeness, parser and
serialization checks. The review found an additional gap: those checks do not
currently prove that all required translator-policy commands were emitted.
No Mastercam SDK or STEP Tools runtime is needed for the proposed changes.

## Pinned baseline and review method

- Upstream `master`: `edca56a70ed658243e142db842bbbe33add7948e`, dated
  2020-09-19, tree `3b9b06324dbe058f4081e41282fbefcecb569103`. The repository
  is public and not archived; the included license is Apache-2.0. Its latest
  source change to `ap238export.cpp` is `a3bf0605f3ce98e6b8108d172701029ab43206a9`,
  dated 2016-02-06. Do not read the old README's description of X9 as a claim
  about current Mastercam releases. [S1, S3]
- Captured all 12 tracked files, 114,693 bytes; verified each file against its
  pinned Git blob and recorded SHA-256 hashes. Reviewed the export/control
  loop, coordinate and arc mapping, tool-loading and selected feature/grouping
  routines, build configuration, license and documentation. This was not a
  complete functional validation of every feature helper.
- The project imports a STEP Tools COM type library and uses Mastercam SDK
  headers/libraries. The project file contains fixed X8/X9 installation paths
  and the `v120` toolset. The plugin and external SDKs were **not built or run**.
  The tracked tree has no automated tests or exported STEP-NC fixtures. No
  compatibility with its actual generated files has been established. [S1, S4]
- Target: clean local and remote `main` at
  `3e77f604d12041bb00cd198a56ba5213d6fd53ab`, version 0.10.0. Its exact-head
  [CI run 36493255697](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36493255697)
  was successful when checked. Existing detailed validation is in
  [validation.md](validation.md). The full suite was not rerun for this
  documentation-only review.
- Evidence directory:
  `D:\dev\Next-Nc research\github-translators-2026-09-28\evidence\steptools--mastercam-stepnc\detailed-review\`.
  It contains the clean `source` checkout, metadata, file manifest,
  `target-probe.cjs`, `target-probe.json` and synthetic inputs/plans/G-code.
  Probes ran on Node 24.13.1 and restored the temporarily instrumented emitter
  after each case. They did not modify production source or execute G-code.

## What transfers from the exporter

| Upstream observation | Current target comparison | Decision |
|---|---|---|
| Operation/tool loading resets first-path handling; rapid, linear and arc handlers track feed and spindle state separately. [S2, S5] | The target has separate requested-value caches and emitted command state. M6 invalidates emitted state but coolant suppression can still use the earlier requested `off`. [T1, T2] | Reconcile suppression with state invalidation and explicitly restore the post-M6 policy. |
| Feed emission compares scalar feed values; spindle emission compares RPM while direction is read separately. [S5] | The target already compares full spindle/feed objects and audits source-path process state. | Keep that stronger behavior. Test equal numeric values with different modes/directions; do not import the scalar caches. |
| Arc handling distinguishes XY/YZ/ZX and changes direction handling with the plane normal. WCS/tool-plane records can be represented as orientation or applied to coordinates. [S6] | Accepted inputs already use resolved WCS coordinates, positive principal arc normals, X radius on the lathe, and a fixed +Z milling tool axis. | Retain those explicit conventions and existing all-plane geometry tests. Do not copy Mastercam-specific coordinate permutations or add a second transform. |
| Left/right cutter contact is distinct from tool-center paths. [S7] | The target consumes Fusion-computed tool-reference paths and emits G40. | Keep unsupported compensation rejected; introducing G41/G42 would change the profile and machining responsibility. |
| Nested workplans, external operations and module saving preserve more than a flat stream. [S8] | The supported profile deliberately has one workplan and an ordered executable graph; unresolved executable semantics are rejected. | Defer general hierarchy/module import. Never silently flatten, skip external operations or follow file paths from untrusted input. |
| The exporter supplies stock, milling tool geometry and selected manufacturing features. Lathe tools use an all-zero generic geometry definition in this code. [S9] | Current files do not contain a validated stock/holder/feature model, and LinuxCNC owns actual tool offsets. | Do not infer collision clearance or physical tools from these routines. Rich descriptive metadata would need a separate producer/profile design. |

The exporter source is a useful source of test scenarios, not a geometric or
modal-state oracle. There are material qualifications: its source-change commit
explicitly disables unfinished `cc3` functionality after crashes; some drill
cycle paths remain unimplemented, and probing-like point operations can be
ignored. Mastercam SDK semantics and the COM implementation were not tested
here. These observations do not prove a particular Mastercam export is wrong.
They support independently implementing the bounded improvements. [S3, S10]

## Target experiments

### M6 state restoration is incomplete in the current command model

Four ordinary synthetic jobs (lathe/mill x mm/inch), each requesting coolant
off, showed the same behavior:

- After M6, the first reviewed approach has unknown spindle direction and
  coolant in the command-state map.
- The first cutting move has its requested spindle established, but coolant
  remains unknown even though the source requests off.
- Both completeness and serialization audits pass. This is an explicitly
  documented v0.10.0 allowance, not a hidden claim that unknown means off. [T2–T4]

The cause is visible in the target: M6 invalidates emitted modal knowledge;
`requested.coolant` remains `off` from the earlier stop sequence, so the later
off request emits nothing. The path audit allows null coolant for an off
request. [T1–T3]

Normal LinuxCNC M6 stops the spindle and normally leaves coolant unchanged.
Our earlier M9 therefore establishes off for that normal sequence. The probe
does **not** show a malfunction of standard M6. The target deliberately models
custom M6 effects as unknown, however, and can explicitly reestablish its
commands afterward rather than carry that uncertainty into the approach/cut.
This would still not prove physical feedback or remap behavior. [L1]

### Required policy commands can disappear without failing either audit

For each machine/unit pair, a research wrapper omitted exactly one structured
emitter call in each of these scenarios. The wrapper omitted the command and
its formatted line together, leaving the rest of translation unchanged.

| Deliberately omitted command | Accepted cases |
|---|---:|
| Spindle stop before final machine-coordinate retract | 4/4 |
| Coolant off before final machine-coordinate retract | 4/4 |
| Spindle stop before a later retract/tool-change boundary | 4/4 |
| Spindle stop before an in-toolpath direction reversal | 4/4 |
| **Total** | **16/16** |

All 16 mutated candidates reported completeness and serialization `passed`.
These were intentionally corrupted outputs; the normal emitter does issue
the tested commands. The finding concerns missing audit coverage, not evidence
that ordinary v0.10.0 translation randomly drops commands. [T1, T3]

The current completeness traversal consumes movements, dwells and plan
waypoints. At waypoints it verifies coordinates/order/provenance but does not
enforce the surrounding stop/off policy. The text audit faithfully checks each
remaining line against each remaining command; it cannot detect an instruction
missing from both. A later M2 also does not prove the required state existed
before the earlier final retract. [T3, T5, L1]

### Equal values already preserve different meanings

Four additional synthetic scenarios verified 600 RPM clockwise changing to
600 RPM counterclockwise, and feed 0.2 per minute changing to 0.2 per revolution,
then rapid followed by cutting at the same per-revolution feed. All preserved
the requested state; the reversal included a spindle stop. This is existing
correct behavior to retain, not an implementation defect. Extend the durable
regressions around these exact cases instead of replacing the state model.

## Implementation sequence

### 1. Reestablish known state after actual tool changes

Keep M6 invalidation: do not assume that a remap preserves positions or modes.
After each emitted `toolChange`, explicitly establish spindle 0 stopped,
coolant off and RPM mode at zero speed (`M5 $0`, `M9`, `G97 S0 $0`) before
any reviewed work approach. Keep the existing units/plane/distance-mode reset,
temporary-offset clearing and explicit WCS/H selection. Never infer position
or skip a required approach because the tool change was followed by resets.

Update requested-value suppression at the same boundary so it cannot suppress
a required command based only on pre-M6 requests. Preserve the distinction
between an actual M6 and a retract with the same selected tool. Checked
`continue` and `link` boundaries retain compatible spindle/coolant state;
do not add stops to them or issue unnecessary tool changes.

After restoration is in place, remove the path-audit exception accepting null
coolant for a source off request. Reports should still show unknown state
immediately after M6, then known **commanded** state after the explicit resets.
Historical archives keep their original uncertainty. Requested source values,
measured feedback and emitted commands remain distinct.

Candidate files: `src/translate.js`, `src/linuxcnc-output.js`,
`src/execution-audit.js`, `test/process-summary.test.js`,
`test/pipeline.test.js`, semantic fixtures and relevant documentation.

**Acceptance:** both machines/units, first and later M6, off/flood/mist,
independent T/H records, and same-tool boundaries. The first approach after
M6 has an explicit stopped/off command state; cutting coolant exactly matches
the source. Removing the new restoration must fail validation. Preserve exact
cutting/transition coordinates, operation order, feeds, CSS conversion/caps,
program fingerprints and plan binding.

This is an intentional G-code policy change: additional reset commands are
expected after M6. Review those exact differences and update reference programs
and golden hashes only for that change. Do not claim byte-identical G-code for
this step or blanket-refresh unrelated golden outputs.

### 2. Independently audit translator policy before publication

Add `src/policy-audit.js` to check obligations across the structured command
stream using the decoded program, validated plan and operation/path boundaries.
Do not derive success solely from emitter-maintained `modalState`, command
counts, text equality or a list of commands supplied by the emitter itself.
Track the small set of relevant policy states independently and evaluate them
where each obligation becomes due:

- Startup and each machine-coordinate retract: the required stop/off/zero-RPM
  preparation precedes the first waypoint, rather than appearing later.
- Actual M6: invalidation is followed by required restoration before the
  work-coordinate approach; initialization/WCS/H ordering remains valid.
- Operation entry: requested initial process state is established at the
  proper boundary, even if the first path subsequently changes that state.
- Spindle reversal: the existing explicit-stop policy is preserved before
  the opposite direction is started; changing only RPM does not invent a
  reversal obligation.
- Compatible continue/link: the approved inherited state remains in force
  through the connection; the audit does not demand retract-style shutdown.
- Program end: preparation precedes final retract, with offset cancellation
  and a single terminal M2 in the established order.

Validate obligations, not a verbatim second copy of the emitter. Known state
may satisfy an obligation without a redundant command, but unknown state may
not be treated as satisfied. Use source/plan context for owning operations and
waypoints; translator policy must not invent a STEP record.

Run this audit after existing completeness and before final serialization.
Return `execution.policy` with schema `linuxcnc-next-nc/policy-audit/1`, actual
checked-boundary counts and scope. Add a real `policy` stage to validation
coverage at that point. Failures use `INTERNAL_ERROR`, stable `POLICY_*`
invariants, phase/line or boundary context, expected obligation and observed
state. CLI/preflight/filter retain empty output on failure and existing local
diagnostic archives. Older reports show absent policy coverage as not recorded;
general EXPRESS and controller commissioning remain unchecked.

Candidate files: new `src/policy-audit.js`, `src/translate.js`,
`src/validation-coverage.js`, `src/report.js`, `src/errors.js`,
`test/pipeline.test.js`, `test/serialization.test.js` and CLI/report tests.

**Acceptance:** all 16 demonstrated omissions fail this new audit. Also reject
late/reordered resets, resets for the wrong boundary and early spindle restart
before a retract. A command missing from both text and source map must still
be caught. Valid source paths and continue/link behavior must remain accepted.

### 3. Make the state scenarios and independent checks durable

Promote the review's synthetic cases into tests, adding only coverage absent
from the existing suites. Use hand-authored expected commands/canonical events;
the unexecuted Mastercam plugin is not the expected-output generator.

Cover equal-speed reversals, equal-value G94/G95 changes, rapid-to-feed with an
unchanged numeric feed, changes occurring only on a dwell, CSS-cap-only changes
and RPM/CSS changes with equal emitted S values. Apply common cases to both
machines/units and CSS cases to the lathe. Include first/later tool changes and
retract, continue and link boundaries, checking process-command timing as well
as final cutting state.

Keep existing positive-normal arc, full-circle, unit-conversion and exact
coordinate tests. Add targeted negative tests only where missing for an
unsupported orientation/normal, helix or compensation request. These should
continue to reject; this plan does not add arbitrary transforms or 5-axis
machining. Any STEP-level source case must be deliberately authored within the
declared input contract, not assumed to be a real Mastercam export.

For fault injection, remove/reorder both commands and their formatted output,
then verify the independent policy check catches the fault. Also inject an
emitter state-map inconsistency to ensure the auditor does not merely trust
the supplied modal snapshot. JSON/HTML must identify the failed stage and leave
dependent serialization unchecked, with escaped values and no partial files.

Run the existing Windows/Linux Node matrix, independent parser comparisons,
Python helper checks and offline LinuxCNC suite. Extend hand-authored native
reference programs for the deliberate post-M6 resets and new state cases.
Preserve the current printed-canonical tolerance and exact-number Node checks.
An offline standard interpreter does not test a custom installed M6 remap;
retain that explicit commissioning limit.

## Deferred work and reuse boundary

- General nested workplans, external modules and feature-based cycles require
  a separate input-profile design, complete ordering/reference rules and
  independently generated files. Keep unsupported executable content rejected.
- Stock/holder/tool-geometry metadata would require changes in the Fusion
  producer and its format contract. It must not become fabricated tool-table
  entries, automatic safe paths or proof of collision clearance.
- Do not import Mastercam-specific coordinate permutations, compensation,
  drilling depths/retract heuristics, or scalar feed/RPM suppression logic.
  Fusion retains CAM decisions; LinuxCNC retains controller configuration,
  tools/offsets, interpretation and motion control.
- No upstream C++ code or external SDK is incorporated by this plan. If a later
  change copies Apache-licensed source, preserve its license/attribution and
  identify modifications; the repository license does not package the required
  Mastercam or STEP Tools SDKs.
- No new machining options, runtime dependencies, restart controls, machine
  configuration changes or speed/quality claims follow from this review.

Deliver state restoration first, independent policy verification second, with
focused tests and documentation alongside each. The planned benefit is known
commanded state and detection of missing preparation/shutdown instructions;
it is not reduced cycle time or a claim of physical machine acceptance.

## Pinned sources

- **S1:** [Upstream repository and README](https://github.com/steptools/mastercam-stepnc/tree/edca56a70ed658243e142db842bbbe33add7948e), [Apache license](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/LICENSE.txt).
- **S2:** [Operation/tool loading and first-path reset](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L387-L451).
- **S3:** [2016 source change disabling unfinished cc3](https://github.com/steptools/mastercam-stepnc/commit/a3bf0605f3ce98e6b8108d172701029ab43206a9).
- **S4:** [Build project](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.vcxproj), [COM import](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L20-L36).
- **S5:** [Rapid state/first point](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L690-L790), [linear spindle/feed suppression](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L794-L825).
- **S6:** [Arc mapping](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L974-L1082), [tool-plane/WCS orientation](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L1198-L1235).
- **S7:** [Cutter-contact compensation](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L703-L735).
- **S8:** [Nested workplans and external operations](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L318-L383), [module saving](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L1263-L1269).
- **S9:** [Block stock](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L246-L266), [milling tool definitions](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L1337-L1367), [lathe tool fallback](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L1507-L1543), [hole feature extraction](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L1580-L1652).
- **S10:** [Ignored point operation](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L738-L741), [unimplemented cc3 drill-cycle branch](https://github.com/steptools/mastercam-stepnc/blob/edca56a70ed658243e142db842bbbe33add7948e/ap238export.cpp#L1127-L1131).
- **T1:** [Current state emission and tool-change sequence](https://github.com/markmounteer/LinuxCNCNext-NC/blob/3e77f604d12041bb00cd198a56ba5213d6fd53ab/src/translate.js#L43-L77).
- **T2:** [Requested state and M6 invalidation](https://github.com/markmounteer/LinuxCNCNext-NC/blob/3e77f604d12041bb00cd198a56ba5213d6fd53ab/src/linuxcnc-output.js#L26-L58).
- **T3:** [Current completeness audit](https://github.com/markmounteer/LinuxCNCNext-NC/blob/3e77f604d12041bb00cd198a56ba5213d6fd53ab/src/execution-audit.js#L45-L133).
- **T4:** [Existing unknown-coolant regression](https://github.com/markmounteer/LinuxCNCNext-NC/blob/3e77f604d12041bb00cd198a56ba5213d6fd53ab/test/process-summary.test.js#L39-L51), [documented command-state scope](https://github.com/markmounteer/LinuxCNCNext-NC/blob/3e77f604d12041bb00cd198a56ba5213d6fd53ab/docs/command-state.md).
- **T5:** [Final serialization audit](https://github.com/markmounteer/LinuxCNCNext-NC/blob/3e77f604d12041bb00cd198a56ba5213d6fd53ab/src/gcode-audit.js), [accepted geometry frames](https://github.com/markmounteer/LinuxCNCNext-NC/blob/3e77f604d12041bb00cd198a56ba5213d6fd53ab/vendor/fusion360next-nc/inspect.js#L185-L236).
- **L1:** [LinuxCNC spindle, M6 and coolant behavior](https://linuxcnc.org/docs/stable/html/gcode/m-code.html#_m6_tool_change), [program-end effects](https://linuxcnc.org/docs/stable/html/gcode/m-code.html#_m2_m30_program_end).
