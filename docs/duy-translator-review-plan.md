# Duy247 STEP-NC-Translator: review and improvement plan

Status: reviewed and planned on 2026-09-28 against LinuxCNCNext-NC v0.8.0. The improvements below are **not implemented** by this document.

## Recommendation

Use [Duy247/STEP-NC-Translator](https://github.com/Duy247/STEP-NC-Translator), sixth in the saved execution-side ranking, as a source of failure cases. Its Windows Forms prototype reads STEP-NC through an external SDK, creates a text summary, then converts that summary into G-code with regular expressions. It is not a suitable backend to import for either our XZ lathe or fixed-axis XYZ mill.

Two improvements are justified:

1. Automatically verify the **actual final G-code text** against the validated command records before publishing it.
2. Allow a review report to check whether an explicitly selected saved G-code file matches its recorded candidate hash.

These improve translation assurance and diagnosis. They do not promise shorter programs or faster machining. Neither requires new Fusion machining properties or copies of LinuxCNC's machine configuration.

## Verified baseline and scope

- Upstream `main`: `1ec2f08048a886f4b44ee21f7819c082378237fc`, dated 2024-02-26; tree `83d4193718f7f0916406443a833853459ca5e809`. GitHub does not mark the repository archived. Its description identifies a summer-practice project. No README or repository-wide license was found in the inspected tree. [S1]
- Target: clean local and remote `main` at `5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3`, version 0.8.0.
- Captured 10 selected source/configuration files, 49,163 bytes, at the upstream commit. Each file was checked against its Git blob hash; the manifest also records SHA-256. Reviewed the complete 591-line `Form1.cs`, project configuration and relevant designer UI definitions. This was not a review of every generated file or bundled dependency.
- Local evidence: `D:\dev\Next-Nc research\github-translators-2026-09-28\evidence\Duy247--STEP-NC-Translator\detailed-review\`. This contains `metadata.json`, `manifest.json`, source files, `target-serialization-probe.cjs` and its JSON results.
- Upstream was **not built or run**. No bundled executable or SDK binary was loaded. Findings about upstream behavior below are source analysis, not reproduced cutting results.
- Reran the target Node suite: **62 passed, one POSIX-only group skipped on Windows, zero failed**. Rechecked the existing [CI run at the exact target commit](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36468956731): all seven jobs succeeded. Native LinuxCNC tests were not rerun for this documentation-only review; their existing scope is recorded in [validation.md](validation.md).
- The fault probes used synthetic fixtures in memory. No private job files were published and no G-code was executed or sent to a controller.

## What upstream teaches us

The interface separates reading, tool/path inspection and G-code export. That is a useful review workflow, already represented here by preflight and HTML reports. The implementation also shows why a human-readable summary must not become the authoritative machining model. [S2]

| Source finding | Implication for our translator |
| --- | --- |
| Uses a fixed summary file without input binding; export appends shutdown text after helpers catch failures. [S3] | Bind artifacts; preserve complete validation before publication. |
| Regex expects integer quantities and specific operation names; spindle output forces M4. [S4] | Check final numeric values and direction independently. |
| Cartesian output loses rapid semantics. Arcs use G02, XY radius calculations and unchecked division. [S5, S6] | Verify explicit motion/plane semantics for both machines. |
| Endpoint filtering ignores motion semantics; arc handling is incomplete; coordinates are rounded and locale-sensitive. [S5, S6] | Preserve every source use and exact numbers. |
| Per-point substring searches infer entity types; the UI adds a Z offset. [S6, S7] | Keep indexed references and existing offset responsibilities. |
| Build configuration uses external STEP Tools DLLs, machine-specific paths and .NET Framework 4.5. [S7] | Retain the portable Node backend. |

There is no repository-wide license establishing permission to import this implementation or its bundled components. Use the observations to write independent tests and checks; do not copy source or redistribute binaries. The plan requires no upstream runtime dependency.

## What v0.8.0 already handles

The existing translator has indexed STEP parsing, typed commands, explicit machine/unit/plane semantics, source provenance, per-translation modal state, ordered completeness checks, exact numeric output, aggregate plan validation, optional tool-table snapshot validation, atomic publication, archived diagnostics, process summaries and job requirements. Both-machine geometry tests already cover sign quadrants, unit equivalence, major arcs, full circles and repeated vertices. These are established behavior, not new proposals from this review. [T1, T2]

One remaining boundary is demonstrably unchecked. `auditExecution` checks line count/association and validates command records and recorded state against the decoded program and plan. It does not decode the actual strings in `output.lines`. The final G-code hash is subsequently calculated from those strings; a hash records their identity but cannot establish that they represent the intended commands. [T1]

The isolated probe wrapped `LinuxCNCOutput.emit`, called the original implementation, and changed only one output string. Command records, source maps and modal state remained unchanged. Each mutation was tried on a lathe and mill in mm and inch:

| Injected text-only fault | Cases | Translations returned with completeness `passed` |
| --- | ---: | ---: |
| Reverse G2/G3 on an arc | 4 | 4 |
| Change G95 F0.2 to F0.3 | 4 | 4 |
| Remove G53 from a machine-coordinate rapid | 4 | 4 |
| Reverse M3/M4 spindle direction | 4 | 4 |
| Replace final M2 with a comment | 4 | 4 |
| **Total** | **20** | **20** |

This is evidence of a fault-detection gap, **not evidence that unmodified v0.8.0 normally emits these errors**. Existing independent Node and native interpreter tests remain valuable; they test known fixtures rather than independently parsing every production translation's final text. [T2, T3]

## 1. Verify final serialization automatically

Add a small independent verifier for the translator's own emitted dialect. After the existing source/command audit, construct the final G-code string and pass that exact string to this verifier before hashing it, building successful reports or publishing file/stdout output. Return the same verified string without subsequent rewriting.

The verifier should decode each emitted block and compare its meaning with the associated validated command. It should not invoke `LinuxCNCOutput.emit`, `decimal`, or another production formatting helper to create its expected answer. Keep its scope bounded: it checks our output grammar and its command association, not arbitrary external RS274 programs or physical controller behavior.

Required checks:

- One output block per command, correct order, no embedded extra lines, and exactly one final executable M2.
- Only words allowed for that command; no unexpected executable suffixes, duplicate axis/value words, conflicting G/M codes or unparsed text. Initialization legitimately has several distinct G words and must be checked as such.
- Exact finite numeric agreement with the command's JavaScript numeric value, using the existing signed-zero convention. Reject exponent/comma notation and unintended rounding; do not introduce a machining tolerance to hide serialization differences.
- Correct initialization, units, G8 radius mode, absolute distance mode, incremental arc centres, compensation/cycle reset, G61, G92.1 and post-M6 reassertions.
- G53 only where the command specifies machine coordinates; unchanged work-coordinate axes remain modal. Verify G0/G1, plane changes, G2/G3, I/J/K and full-circle P1/end-axis behavior. A lathe keeps XZ radius coordinates; a mill supports XY/XZ/YZ arcs with a fixed tool axis.
- Independent T/H and WCS mappings; G94/G95 values; G96/G97 values and CSS cap; M3/M4/M5 with spindle selection; coolant, dwell and shutdown instructions.
- Comments remain non-executable and follow the existing sanitization/prefix contract. User metadata must not introduce active LinuxCNC comments or extra commands.

A mismatch raises `INTERNAL_ERROR` with a stable invariant identifier, one-based output line, expected command, observed block/tokens, operation/path context where available, and STEP or plan provenance. It is an internal translation failure, not an instruction to change Fusion compensation or another unrelated user setting. All CLI paths must retain structured diagnostics and emit no partial G-code or successful preflight result. This check is automatic, with no bypass property.

Add an optional versioned `execution.serialization` result to new inspection reports, distinct from the existing completeness result. State its limited scope. Older archives render it as “Not recorded”; do not infer that an old successful translation had this check. Keep profile versions, plan schemas and fingerprints unchanged.

**Acceptance:** turn all 20 probes above into rejected regressions. Extend the same text-only mutation harness for changed axis/centre values, wrong plane/units/T/H/WCS, omitted CSS cap, coolant/dwell changes, malformed numbers, executable suffixes and active comments. Exercise both machines/units, major/full circles, repeated endpoints, fractional G95 feeds, small decimal values and mode-only final G94. Verify CLI `translate`, `preflight` and `filter` fail before publication and preserve existing files. Keep all normal golden G-code byte-identical and all current semantic tests passing. Use the existing native interpreter matrix as an independent backstop, with its documented precision limits.

Candidate files: new `src/gcode-audit.js`, integration in `src/translate.js`, focused tests alongside `test/pipeline.test.js`, additive HTML rendering in `src/report.js`, and validation documentation. Keep the algorithm linear in emitted text size; avoid a second full command history or per-line rescans of the program.

## 2. Bind an optional saved-file check to the report

The current report displays hashes but does not read a selected G-code file. The AXIS helper explicitly asks the operator to establish the relationship to the loaded job. Preserve that distinction while offering a useful offline identity check. [T4]

Proposed invocation:

```text
nextnc-linuxcnc report diagnostic.json --gcode job.ngc --output NEW_REPORT.html
```

`--gcode` selects a file for review; it is not a machining option. Read it as bounded regular-file bytes and compare SHA-256 with **`inspection.traceability.gcodeSHA256`**. Do not use top-level `outputSHA256`: in a preflight archive that hashes the JSON preflight result, not the candidate G-code. Hash bytes without normalizing line endings, a BOM, comments or whitespace; exact identity is the contract. [T4]

The report should prominently distinguish:

| Result | Meaning and CLI behavior |
| --- | --- |
| Match | Selected bytes match the recorded candidate; exit 0. This proves identity relative to that archive, not authenticity, successful execution or current controller loading. |
| Mismatch | Write a clearly marked review report and exit 1. Its source map still describes the archived candidate, not the selected file. Do not present line associations as verified for that file. |
| Not recorded / invalid expected hash | When a file was explicitly supplied, write an unverified report and exit 1. Do not fall back to another hash or a filename match. |
| Not checked | No `--gcode` argument: preserve existing report behavior and exit 0 after successful rendering. |

Unreadable, non-regular or excessive-size input is an ordinary failure with no success report. Use a fixed documented resource limit rather than another user setting, and enforce it on the bytes actually read. Record the selected path, observed hash, expected hash and check time in the derived HTML. Never rewrite the archived diagnostic, its `latest` indexes or the G-code. Preserve exclusive output creation and escape all supplied metadata.

Read only the explicitly selected file; do not follow paths stored inside an archive automatically. The result describes the bytes read at that time and cannot establish what AXIS currently has loaded. Leave the existing AXIS helper unchanged initially and document the optional CLI workflow.

**Acceptance:** matching translations and preflight candidates, changed coordinates, a stale same-named job, newline/BOM-only differences, missing/malformed legacy hashes, read failures, hostile metadata and an existing HTML destination. Assert that report generation never changes diagnostic indexes or input files. This check does not parse or execute arbitrary supplied G-code; step 1 applies within translation.

Candidate files: `bin/nextnc.js`, a small `src/artifact-identity.js` helper, `src/report.js`, `test/review.test.js`, README usage and `docs/axis-report.md`.

## Delivery and boundaries

Implement step 1 first, then step 2 as a separate reviewable change. Each must retain lathe and XYZ mill coverage. Run the Windows/Linux Node matrix and native LinuxCNC suite before releasing behavior changes; record actual results rather than treating this plan as delivered.

Fusion continues to own toolpath generation and compensation decisions. The reviewed execution plan supplies mappings and explicit transitions. LinuxCNC owns real offsets, machine state, motion planning and feedback. No Fusion post change, SDK installation, alternate tool table, Z-offset slider, rounding option or controller-style selector is needed for this work.

Do not adopt upstream's regex intermediate format, XY-only arc reconstruction, endpoint-only deletion or assumed spindle direction. Full AP238 import, feature-derived cycles, blending policies, new GUI previews, live controller integration and adaptive machining remain outside this plan. The reviewed prototype provides no new basis to implement them for either machine.

## Pinned sources

- **S1:** [Upstream tree](https://github.com/Duy247/STEP-NC-Translator/tree/1ec2f08048a886f4b44ee21f7819c082378237fc).
- **S2:** [Read/inspect workflow](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L23), [UI controls](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.Designer.cs).
- **S3:** [Fixed summary file](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L77), [export and error handling](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L267).
- **S4:** [Name/number matching and spindle output](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L357).
- **S5:** [Rapid marker](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L228), [Cartesian and arc conversion](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L396).
- **S6:** [Endpoint filtering](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L470), [XY centre calculation and rounding](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L516).
- **S7:** [Entity text scanning](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/Form1.cs#L193), [project dependencies](https://github.com/Duy247/STEP-NC-Translator/blob/1ec2f08048a886f4b44ee21f7819c082378237fc/STEP-NC%20Forms/STEP-NC%20Forms.csproj).
- **T1:** [Current completeness audit](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/src/execution-audit.js#L122), [formatter](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/src/linuxcnc-output.js), [translation pipeline](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/src/translate.js).
- **T2:** [Existing validation evidence](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/docs/validation.md), [geometry invariants](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/test/geometry-invariants.test.js).
- **T3:** [Existing audit fault tests](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/test/pipeline.test.js#L85), [synthetic fixtures](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/test/support/semantic-fixture.js).
- **T4:** [Report CLI and archive hashes](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/bin/nextnc.js#L41), [AXIS report identity limitation](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5d013873ab0d703dbfa71eac8a5b27d6e8f71bd3/docs/axis-report.md#L17).
