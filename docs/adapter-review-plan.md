# Plan: improvements informed by STEP-NC-to-GCODE-Adapter

Status: implemented in v0.5.0 with the additional user-requested XYZ milling scope. The historical review and proposed sequence below explain the design.

Implemented: HTML archive reports, aggregate plan diagnostics, shared filter/doctor configuration, shell regression checks and an optional AXIS report helper. XYZ milling required a new format profile and Fusion producer v0.2.0; legacy turning model fingerprints remain compatible. No live AXIS configuration or hardware has been modified. The optional helper still needs visual acceptance in the user's AXIS simulation. See [milling](milling.md), [preflight](preflight.md) and [AXIS helper](axis-report.md).

Reviewed 2026-09-28 UTC:

- Upstream: [meduag/STEP-NC-to-GCODE-Adapter](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/tree/21a869885d9a34b56d1f06f796e434d395c634b0), commit `21a8698`, dated 2017-03-30.
- Target: [LinuxCNCNext-NC](https://github.com/markmounteer/LinuxCNCNext-NC/tree/f9276de97737d6213f235fa11a3e620ce64d6848), v0.4.1, commit `f9276de`.
- Local checkout matched the target's current GitHub HEAD and was clean before this document was added. All 19 existing Node tests passed during this review. The upstream Java/GTK application and LinuxCNC GUI were not run. The standalone LinuxCNC interpreter tests were inspected, not rerun in this Windows session.

## Decision

Use the adapter as a reference for source visibility, job setup presentation and integration failure cases. Implement the selected ideas independently in the existing Node translator. Its feature-based milling generators are not compatible replacements for this project's resolved XZ turning toolpaths.

Preserve the present ownership of information: Fusion supplies ordered compensated cutting paths and process settings; the execution plan supplies reviewed controller mappings and transitions; LinuxCNC owns its tool table, offsets, preview, interpretation and motion control. This plan introduces no additional machining settings in the CPS or translator.

No explicit repository-wide reuse license was identified upstream. Directly importing its Java/GTK code is not part of this plan. Public source availability alone does not establish permission to incorporate it into our MIT repository.

## What the upstream implementation actually contributes

| Source finding | Current target behavior | Decision |
| --- | --- | --- |
| The upstream AXIS menu launches a GTK source viewer. The viewer reads one hardcoded `Code_original.txt` path. [U1] | Inspection and preflight expose JSON, with operation/path information and a G-code source map. There is no human-oriented report renderer. [T1] | Adopt source visibility as a portable, read-only job report bound to the actual input and output hashes. |
| A separate configuration file repeats safe-plane, machine-feed, cutting-feed, depth and step parameters. Its reader depends on working-directory paths and positional values. [U2] | The CLI accepts an explicit plan/table; the shell filter resolves the plan through environment/default paths. [T2] | Add an installation diagnostic using the existing settings. Preserve Fusion/LinuxCNC as the authoritative sources. |
| The adapter identifies/groups milling tools, deletes the configured old tool-table file and writes generated tool records. [U3] | Preflight reads a supplied table snapshot, checks independent T/H records and aggregates missing records. It does not write the table. [T3] | Adopt a tool-use summary, displaying existing mappings and affected operations. Keep the table read-only. |
| The emitter writes numbered G-code directly to stdout and formats many values to four decimal places. [U4] | The target validates the entire program before output, preserves decoded coordinates and supplies one-based line maps. [T4] | Retain the target's emission design. Use the upstream implementation to identify regression cases, not as an output oracle. |
| Tool changes contain fixed XY/clearance conventions; generators select `G17` and can emit cutter compensation. [U5] | The target uses XZ/radius mode, reviewed transitions, explicit mappings and Fusion-computed compensation. [T4] | Exclude these machining algorithms and defaults. |

Upstream record indexing and token-position parsing also reinforce the value of our existing indexed Part 21 reader. Replacing that reader, adding another CAM planner, or introducing operation reordering has no demonstrated benefit here.

## Implementation sequence

### 1. Readable job and diagnostic report — first deliverable

Add a report command that renders an existing archived diagnostic JSON into a self-contained HTML file. The proposed interface is:

```text
node bin/nextnc.js report diagnostic.json --output new-review.html
```

The renderer must not regenerate G-code, interpret STEP a second way, or execute report content. Start from the existing diagnostic schema and reject unknown incompatible schemas.

Show:

- Input identity, time, translator version, input/plan/table hashes and program fingerprint where available.
- Whether the record describes inspection, preflight, translation or failure. A preflight candidate must not be presented as an already-written program.
- Ordered operations, exported tool/offset identifiers, mapped T/H and WCS, entry/exit positions, feed modes, spindle/CSS caps, coolant and boundary decisions, using data actually present in the report.
- A grouped tool-use table showing the operations using each mapping, with table-check status. Missing data is explicitly unavailable, rather than reconstructed from assumptions.
- Links within the report from an operation to its generated line range and recorded motion/path details. Display the candidate G-code hash beside the source map.
- Clear failure context and an actionable correction location: Fusion operation, execution-plan field, table file, or installation environment.

Candidate files: new `src/report.js`; `bin/nextnc.js`; `src/diagnostics.js` only if small additive metadata is necessary; `docs/preflight.md`; new `test/report.test.js`; `package.json` test command.

Keep existing JSON commands and the filter's stdout contract unchanged. Save the HTML only at the explicit new output path, using the existing exclusive-write mechanism. A user can generate it from the existing standard diagnostic archive; automatic HTML generation on every translation is unnecessary.

Acceptance: both success and failure archives render; absent plan/table/source-map fields are handled; operation names and paths containing HTML/script-like text remain plain text; no network assets or scripts are loaded; existing files remain unchanged; generating a report does not alter any G-code bytes. No 3D preview, machine controls or run-from-line function is needed—LinuxCNC already provides preview and execution.

### 2. Aggregate independent preflight problems

`validatePlan()` currently throws at its first failing check, so a template with several unmapped tools, work offsets and missing approach paths usually requires repeated runs. The tool-table checker already aggregates missing records; extend the same user benefit to independent plan problems. This is a target improvement motivated by the review, not a capability claimed for the Java adapter.

Introduce a structured plan-issue collector used by both preflight and translation. Preserve the existing primary error code/context for compatibility and attach the ordered issue list additively. Include section, operation, field path, cause and correction guidance.

Dependency rules are essential:

- Invalid STEP structure, unknown executable semantics, incompatible plan schema, wrong program fingerprint or wrong units prevent dependent checks.
- Once the program and plan containers are valid, independent tool/WCS/transition errors can be collected across sections.
- A continuation/link check requiring an invalid mapping is marked not checked with that dependency; it must not invent a result.
- Unexpected implementation exceptions remain fatal. Do not implement aggregation by catching every exception and continuing.
- Any blocking issue leaves translation stdout empty and creates no output program. Both commands still use the same checks.

Candidate files: `src/plan.js`, `src/errors.js`, `src/translate.js`, `bin/nextnc.js`, `test/preflight.test.js`, `test/cli.test.js`; share the report presentation from step 1.

Acceptance: a synthetic multi-operation job reports all independent missing mappings and transitions in one run; malformed inputs do not produce speculative downstream findings; correcting only one issue leaves the others visible; successful G-code remains byte-identical to the baseline; schema 1/2/3 plan behavior is preserved. Existing aggregation of missing T/H records remains intact.

### 3. Diagnose the installed filter using its real configuration

Add a read-only `doctor` command. Its scope is installation and file resolution, not machine acceptance. Report the Node version, translator/wrapper location, the effective plan and optional table paths, the source of each selection (explicit/environment/default), file readability, and the diagnostic directory.

Move filter path/default resolution into a single small JavaScript helper used by both the filter entry point and doctor. Avoid maintaining competing shell and JavaScript implementations of the default-path rules. Keep the wrapper thin and give a concise stderr error if Node cannot start. Changes to an existing output contract should be covered before replacing the launcher behavior.

Do not make a second machine configuration. The first version should not attempt a partial LinuxCNC INI parser, evaluate configuration expressions, discover a live controller, write a tool table, or test HAL. State that those checks were not performed. A writable-directory probe, if needed, must be an exclusive temporary file in the resolved diagnostics directory and remove only that exact file.

Candidate files: new `src/filter-config.js` and `src/doctor.js`; `bin/nextnc.js`; `bin/nextnc-filter`; `docs/linuxcnc.md`; new `test/filter.test.js` and doctor tests.

Acceptance: non-project working directories; spaces and Unicode in paths; XDG/default/environment precedence; missing plan/table; unreadable files; unsupported Node; inaccessible diagnostics directory. Test the actual shell wrapper on Linux. Doctor and the wrapper must report/use the same resolved inputs, and any installation failure before translation must emit no G-code.

### 4. Optional AXIS access after the portable report works

Offer a small configuration-specific action to open the report for an explicitly selected diagnostic record. Current LinuxCNC documents `USER_COMMAND_FILE` for per-configuration AXIS customization. Use that extension point if an AXIS shortcut is useful; do not replace the installed `axis.tcl` as the upstream repository does. [L1]

The hook opens the read-only report; it does not edit mappings, touch tools, start motion or authorize execution. An arbitrary global `latest.json` must not be labeled as the currently loaded program. Show the input identity/hash and require a matching record before claiming association with a selected job. If the installed AXIS version cannot expose a reliable association, retain explicit report selection.

Candidate files: optional `integrations/axis/` helper/example and `docs/linuxcnc.md`. Validate with an isolated simulation configuration and the intended installed LinuxCNC version. This is a later optional deliverable, not a prerequisite for steps 1–3.

LinuxCNC also supports filter progress on stderr. Add progress only if measured input sizes make loading noticeably slow, and derive it from completed work. Do not emit invented percentages or put progress text in G-code stdout. [L1]

### 5. Regression coverage throughout the changes

Use newly authored synthetic fixtures; the upstream milling outputs are not expected outputs for our turning translator. Add only missing coverage after checking the existing tests:

- Exact tool identities with multiple digits, independent T/H mappings, distinct tools with similar descriptions, and identical mapped tools with changed offsets.
- Repeated input processing without state leaking from a previous run; operation order stays unchanged.
- Locale-independent numeric output and no new rounding or coordinate fitting.
- Actual Linux shell filter failures beyond the currently tested missing-table case: malformed input, missing/wrong plan, file access failure and spaces in paths. Assert empty stdout on validation failure and useful stderr.
- Report/diagnostic isolation for simultaneous jobs. An archive's identity must remain unambiguous even though the global latest pointer naturally changes.
- Run the existing standalone `rs274` cases after changes to the emission/validation path, preserving interpreted arcs, endpoints, feed modes, CSS caps and continuation/link behavior.

No changes to G61/blending, feed values, spindle limits, compensation, operation order, tool-change geometry or the accepted turning profile are justified by this review. The target already suppresses repeated M6 for the same mapped tool and supports checked continuation/link boundaries. Do not present those as new optimizations.

## Delivery boundaries

Implement steps 1, 2 and 3 as separately reviewable changes, each with its relevant tests and documentation. Keep step 4 optional until the actual AXIS version and user preference for an in-GUI shortcut are known; useful work on the first three does not depend on that choice. Fold step 5 into each change, rather than postponing testing to the end.

The first release should improve how a user understands a job and fixes setup problems, with unchanged successful machining output. There is no evidence here that adopting the Java adapter would shorten machining time or improve turning geometry. No Fusion CPS or shared vendored reader change is required for this first scope.

## Source references

- [U1: source viewer](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/GUI%20Axis%20e%20Viewer%20STEP-NC/CodViewerGui.py), and [AXIS menu integration, lines 256–259](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/GUI%20Axis%20e%20Viewer%20STEP-NC/axis.tcl#L256-L259).
- [U2: repeated configuration](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/arqConf.txt), and [configuration reader](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/src/br/UNB/LAB/Integrador/lerDadosArqConfg.java).
- [U3: tool grouping and table replacement](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/src/br/UNB/LAB/GerarCodG/OrdenarFerramentas.java#L320-L354).
- [U4: numeric formatting and generator](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/src/br/UNB/LAB/GerarCodG/Gcode.java), and [stdout emitter](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/src/br/UNB/LAB/FAcessorias/ImprimirSecuenciaLineaNo.java).
- [U5: tool-change movement assumptions](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/src/br/UNB/LAB/GerarCodG/TrocaDeFerramenta.java), and [controller compensation](https://github.com/meduag/STEP-NC-to-GCODE-Adapter/blob/21a869885d9a34b56d1f06f796e434d395c634b0/src/br/UNB/LAB/FAcessorias/EncabezadoGcodeCompenTool.java).
- [T1: current CLI/report output](https://github.com/markmounteer/LinuxCNCNext-NC/blob/f9276de97737d6213f235fa11a3e620ce64d6848/bin/nextnc.js), and [plan validation](https://github.com/markmounteer/LinuxCNCNext-NC/blob/f9276de97737d6213f235fa11a3e620ce64d6848/src/plan.js).
- [T2: current filter wrapper](https://github.com/markmounteer/LinuxCNCNext-NC/blob/f9276de97737d6213f235fa11a3e620ce64d6848/bin/nextnc-filter).
- [T3: table snapshot validation](https://github.com/markmounteer/LinuxCNCNext-NC/blob/f9276de97737d6213f235fa11a3e620ce64d6848/src/tool-table.js).
- [T4: current emission/state handling](https://github.com/markmounteer/LinuxCNCNext-NC/blob/f9276de97737d6213f235fa11a3e620ce64d6848/src/translate.js), and [standalone interpreter/filter tests](https://github.com/markmounteer/LinuxCNCNext-NC/blob/f9276de97737d6213f235fa11a3e620ce64d6848/scripts/test-linuxcnc.js).
- [L1: official AXIS documentation, program filters and configuration-specific customization](https://linuxcnc.org/docs/stable/html/gui/axis.html#_program_filters).
