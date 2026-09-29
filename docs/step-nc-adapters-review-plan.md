# step-nc-adapters: review and improvement plan

Status: **reviewed; implementation is planned**, against LinuxCNCNext-NC
v0.11.0. This change adds documentation only. It does not add an APT, additive,
or general AP238 importer.

## Recommendation

Use [EfrainRodriguez/step-nc-adapters](https://github.com/EfrainRodriguez/step-nc-adapters)
as a reference for a reusable parsing/conversion API and as a source of
interoperability test cases. Its actual STEP-NC samples reveal a useful input
diagnostic gap in this translator. Its Windows STEP Tools generation backend
and machining defaults should not become dependencies or execution policy.

Prioritize three changes for **XZ lathes and fixed-axis XYZ mills**, both mm/inch:

1. Accept bounded Part 21 comments and token whitespace without changing
   geometry, fingerprints or source-location accuracy.
2. Clearly distinguish a recognized Next-NC profile from another STEP-NC file
   that happens to have the same schema name and extension.
3. Provide a small public library API around the existing strict pipeline,
   with published capabilities and common CLI/library regression cases.

These are formatting, diagnostic and integration improvements. There is no
evidence here for a cutting-speed or cycle-time improvement. The current
completeness, policy and serialization audits remain mandatory.

## Pinned evidence and limits

- Upstream `master`: `86a14b8dd3299f46e894a95136e798082febe293`, dated
  2026-06-04; tree `33421ba77ff55f86a51fcca30c7f6ba6e92c220e`. The repository
  has three commits and is not archived. The latest commit adds `AGENTS.md`;
  the preceding code commit is `5db0a794caaf42e82057a112ea4dc28fe072d6e9`.
  GitHub reports no license, and the complete tracked tree contains no license
  file. No permission to redistribute its source or samples is assumed. [S1]
- Captured 19 selected files, 10,779,356 bytes, including the C# source, project
  file, README and all four tracked output samples. Each downloaded file was
  verified against its pinned Git blob and a recorded SHA-256. The manifest
  explicitly lists 11 uncaptured files, principally large inputs and supporting
  material. This is not a claim to have captured every upstream file.
- Reviewed the public API, models, CLI, APT/XML/JSON parsers and corresponding
  generators. The project targets `net8.0-windows`, x64, and a fixed local
  `stepnc_x64.dll` path. This host has .NET runtimes but no SDK, and that DLL
  is absent. The C# application and native generator were **not built or run**.
  No library was installed to work around that limit. Source observations below
  are distinguished from executed target probes. [S2]
- Target: clean local and remote `main` at
  `72f0db74704fe61f6c28c3cf4cbcc104a32c2a5e`, version 0.11.0. Its exact-commit
  [nine-job CI run](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36501925942)
  passed when checked. Existing evidence covers 94 Linux Node groups, 93 Windows
  groups plus one platform skip, 56 independent-parser cases, and 96 native
  interpreted programs. The full suite was not rerun for this documentation
  review; the new probes below ran locally on Node 24.13.1.
- Local research directory:
  `D:\dev\Next-Nc research\github-translators-2026-09-28\evidence\EfrainRodriguez--step-nc-adapters\detailed-review\`.
  It contains metadata, tree, verified source manifest, `capture.py`,
  `target-probe.cjs` and `target-probe.json`. No private jobs were uploaded,
  generated G-code executed, or controller settings changed.

## What transfers, and what does not

| Upstream source observation | Current target comparison | Decision |
|---|---|---|
| `StepNcApi` separates parsing from conversion and exposes reusable functions; models distinguish programs, operations and commands. [S3] | Our core already separates parsing, validation and emission, but the package root exports only `translate`, `decimal` and `comment`. Inspection/template/preflight require CLI use or internal module paths. [T1] | Add a small supported facade; retain the existing core and error detail. A second converter architecture is unnecessary. |
| The tracked STEP-NC outputs contain block comments and rich source header metadata. [S4] | The bounded parser rejects comments before it reaches profile diagnostics. The supported profiles are narrower than their shared `INTEGRATED_CNC_SCHEMA` name. [T2] | Improve lexical formatting support and report profile compatibility explicitly. |
| APT models retain units, feed units and spindle direction, but the generator always calls `Millimeters()`/`SetModeMill()`, emits XY arcs, and forwards numeric feed/spindle values without those unit/direction fields. [S5] | Our lathe/mill paths retain explicit units, feed modes, spindle direction, CSS caps and principal planes. | Preserve our semantics. Do not use this generator as a lathe backend or an expected-output oracle. |
| APT tool loading updates `currentToolId` before flushing the preceding command list; unknown lines advance without a diagnostic. Missing GOTO components default to zero. [S6] | Our operation/tool ownership and exact vertices come from validated source associations; unsupported executable semantics fail. | Add focused A-B-A tool-ownership and incomplete-coordinate regressions; do not import fallback behavior. |
| Additive JSON models retain source/generator metadata and explicit travel/extrusion events, but parsing can skip malformed points and supply default Z; layers are sorted by sequence/Z. [S7] | Current source/plan completeness checks consume every required use and preserve operation order. | Retain provenance and complete consumption. Do not sort machining operations, skip malformed points or infer missing coordinates. |
| JSON generation distinguishes travel from deposition and labels its volumetric-flow-to-spindle mapping; optional reflective calls return null on failure. [S8] | Our spindle fields mean machining RPM/CSS, and mandatory translator checks fail on errors. | Keep quantity meanings explicit. Never interpret extrusion flow as spindle RPM or silently omit required semantics. |

The APT tool-ownership observation follows directly from source ordering: a
`LOAD` for tool B can label accumulated tool-A commands with B before the list
is flushed. That is a source-traced risk, not a claim that this review executed
the C# parser or proved a particular distributed sample was machined incorrectly.
The unknown-line and coordinate defaults likewise describe code paths, not
the outcome of a compiled upstream test.

The upstream project's count summaries and facade are useful, but counts alone
are weaker than this translator's existing ordered-use and state audits. There
is no reason to replace those audits with upstream success flags.

## Executed target probes

### Real upstream samples are rejected before useful compatibility diagnosis

All four unmodified, hash-verified files fail `readProgram` with
`INVALID_NEXTNC: Invalid entity type /`, at source line 3, the first comment.
Each declares `FILE_SCHEMA(('INTEGRATED_CNC_SCHEMA'))`. [S4, T2]

| Tracked sample | Bytes | Records parsed in the research-only comment-masking experiment |
|---|---:|---:|
| `test_program.stpnc` | 11,942 | 131 |
| `testPart-1.stpnc` | 357,196 | 4,061 |
| `square.stpnc` | 3,742,361 | 38,432 |
| `cube_new.stpnc` | 6,544,681 | 67,224 |

For diagnosis only, the probe replaced comments outside quoted strings with
spaces while preserving newlines. All four then passed the bounded syntax
reader, but failed the target's profile-shape check at `#16`, where
`PRODUCT_DEFINITION_CONTEXT.frame_of_reference` is `$` instead of the required
profile reference. None was accepted or translated. They do not declare either
supported Next-NC profile. Removing comments is **not** an import workaround,
and failing our profile contract does not establish that a file violates AP238.

This demonstrates two separate boundaries: lexical presentation and supported
execution semantics. Improving the former must leave the latter enforced.

### Supported programs also fail on presentation-only changes

For each lathe/mill and mm/inch combination, the probe tested:

- A comment after `HEADER;`: rejected as `Invalid entity type /`.
- A comment after `DATA;`: rejected as `Invalid record ID`.
- `DATA ;` instead of `DATA;`: rejected as `Incomplete Part 21 document`.

All **12 variants** were rejected. The already-pinned independent Part 21 parser
accepted all 12 without diagnostics; their normalized headers, records,
parameters and references exactly matched the unmodified baselines. This is
bounded parser-comparison evidence, not general EXPRESS/AP238 certification.
Its verified compiled-parser hash was
`e241a05a73726a6d97f2c87f00f07d3c24e2b920036ccecbf3fefe644d4db06f`.

The four original programs passed. Four controls containing literal
`/* not a comment */` inside a string also passed; a future comment scanner
must preserve that text. Four unknown-profile controls were correctly rejected.
The complete 24 synthetic cases and eight raw/masked sample outcomes are
recorded in `target-probe.json`. No production parser was patched for the probes.

## Implementation plan

### 1. Bounded lexical formatting support

Replace literal newline-based section finding with a bounded, string-aware
scanner for the existing document envelope. Recognize whitespace and `/*...*/`
comments between tokens in the header and DATA section. Comments inside strings
are text; quoted apostrophes and existing STEP escapes retain their meanings.
Never remove text with a blanket comment regex or concatenate tokens across a
comment. Unterminated strings/comments and unexpected executable content remain
errors with original source locations.

Keep one required header, one unnamed DATA section, current header attributes,
record/reference validation, complete input consumption, 32 MiB input bound
and 64-level nesting bound. This is not a proposal to accept ANCHOR/REFERENCE,
multiple DATA sections, modules, arbitrary schemas or external file references.
Explicitly document any still-unsupported lexical constructs.

Preserve original-input SHA-256 and the existing one-based source-location
convention. Formatting can change provenance locations and raw hashes, but must
not change the decoded program fingerprint, reviewed-plan acceptance or G-code
for the same program. Existing negative controls must stay rejected.

The parser is an unchanged vendored copy from Fusion360Next-NC. Make the small
shared-reader change in that source repository, test it there, then update the
vendored copy and `PROVENANCE.md` to the exact tested commit. No Fusion CPS
machining settings or writer/profile changes are required. Do not leave a local
parser fork while claiming it is an unchanged copy. [T2, T3]

Candidate files: Fusion360Next-NC `lib/part21.js` and reader tests; target
`vendor/fusion360next-nc/part21.js`, provenance, parser/CLI tests and isolated
`tools/step-conformance` fixtures. The independent parser stays development-only.

**Acceptance:** all 12 presentation variants decode identically and generate
identical G-code within the same translator version. Against v0.11.0, machining
commands and fingerprints remain unchanged; a release-version banner change
is the only permitted output difference. Test multiline comments, comments
between tokens, quoted delimiter text, doubled quotes, CRLF, indentation,
unterminated comments, fake section markers inside strings/comments, trailing
records and both input/depth limits. Error line/column and source-map records
must point to the actual source, not a stripped copy. The four upstream samples
remain non-translatable after lexical parsing succeeds.

### 2. Profile compatibility and producer diagnostics

Add an explicit, bounded identity check after Part 21 parsing and before the
strict Next-NC field/semantic validators. Read the already-parsed FILE_SCHEMA
and FILE_NAME producer strings and conservatively follow the actual workplan
profile-property association. A profile word in a comment, arbitrary label or
unrelated representation must not identify the program. Missing, unsupported,
malformed, duplicate and conflicting profile declarations need distinct reasons.

For recognizable syntax, report:

- The declared schema and producer/preprocessor strings, labelled as input
  claims rather than verified software identity.
- The declared Next-NC profile, or its absence/ambiguity, and the two supported
  profiles.
- A precise compatibility reason and source record/location when one exists.
  For absence, do not invent a record or operation.
- A correction explaining that a `.stpnc` extension or matching AP238 schema
  name does not make an arbitrary STEP-NC file executable by this translator.

Use an additive versioned `sourceFormat` report and a real `profileIdentity`
coverage stage. Recognition does not mark profile-shape, geometry or execution
validation passed. A recognized profile still runs **all** existing checks.
Malformed supported-profile fields keep their detailed `PROFILE_SHAPE` errors.
Syntax failures must not be reclassified as valid-but-unsupported files.

Prefer a structured `PROFILE_IDENTITY` error with a bounded reason vocabulary
for the new identity failures, retaining existing schema/syntax error categories.
Errors should distinguish regeneration with the supported Fusion post from a
missing separate format adapter; do not suggest changing a plan to repair a
foreign format. Preserve partial completed coverage and available source header
claims in diagnostic archives. Old archives display missing fields as
"Not recorded"; HTML escapes all source text.

Add a versioned, descriptive capability manifest for the two existing profiles:
machine/coordinate conventions, units, accepted motions/planes, feed/spindle
modes, required reviewed plan and unimplemented execution features. Share the
manifest between compatibility messages and the public API. It is not a machine
configuration, feature toggle, schema negotiation mechanism or automatic importer.
Tests must keep it consistent with the actual validators.

Candidate files: new `src/profile-identity.js` and `src/capabilities.js`,
`src/profile.js`, `src/validation-coverage.js`, `src/errors.js`, `src/report.js`,
CLI error archiving, report/identity tests and profile documentation.

**Acceptance:** independently authored same-schema/non-Next-NC fixtures produce
an actionable unsupported-profile diagnostic, never G-code. Supported-profile
files with bad geometry or fields still fail the correct validator. Test spoofed
markers, multiple workplans, conflicting versions, escaped metadata, unknown
schema and truncated input. No identity result authorizes machining or claims
full AP238 validity.

### 3. A public facade that preserves the validation boundary

Expose documented functions from one package entry point, implemented through
the existing pipeline:

- `inspect(text)` for strict supported-profile inspection.
- `createPlanTemplate(text)` for an intentionally incomplete review template.
- `preflight(text, plan, options)` for candidate checks/reporting without
  publishing G-code.
- Existing `translate(text, plan, options)` with its current result/error
  contract.
- `capabilities()` for the versioned supported-format description; expose the
  same description through a read-only CLI command.

Preserve the current root exports, including existing `translate`, `decimal`
and `comment`, so consumers do not break. The facade accepts source text and
uses the same strict reader; it must not accept an arbitrary caller-supplied
decoded model, a "validated" Boolean or a digest as permission to emit code.
Keep parsed models internal. Avoid speculative adapter plugins or a large
intermediate-representation rewrite.

CLI and library methods should share implementation while leaving filesystem
selection, exclusive publication, stdout/stderr handling and diagnostic archiving
in the CLI layer. Library calls must not read local settings, write files, load
controller state or swallow structured errors. Preflight must execute the same
completeness/policy/serialization checks as translation and bind its candidate
hash consistently. No runtime STEP Tools/.NET dependency is introduced.

Candidate files: new `src/index.js`/small core helper if needed, `package.json`
entry point, `bin/nextnc.js`, API/CLI equivalence tests and API documentation.

**Acceptance:** for both machines/units, CLI and library inspection/templates/
preflight/translation agree on fingerprints, diagnostics, coverage, candidate
hashes and exact G-code where applicable. Verify calls share no mutable state
and perform no publication through the library API. Keep all three translation
audits mandatory. Add focused A-B-A tool/offset ownership, missing coordinate,
unknown executable property and state-preservation cases inspired by the source;
reuse the existing all-plane, reversal, feed and M6 tests rather than duplicating
their machinery. Unknown quantities must never default to zero.

## Delivery, reuse and deferred work

Deliver the reader improvement first, identity/capability diagnostics second,
and the facade third, with tests and documentation alongside each. Existing
v0.11.0 post-M6 output is the new behavior baseline: this plan adds no further
machining commands. Run the Windows/Linux Node matrix, independent-parser
comparisons, Python helper checks, example reproducibility and native LinuxCNC
suite before publishing an implementation. Confirm production packaging excludes
development compilers/checkers.

Keep raw upstream samples in local research. Repository/CI regression fixtures
should be independently authored minimal cases; document pinned upstream hashes
and observations without copying unlicensed code or sample bodies into the MIT
package. No upstream C# source is needed for the proposed improvements.

Defer APT import, additive deposition, arbitrary AP238/ARM execution, nested
workplan flattening, module loading, multi-axis machining and stock/tool geometry
inference. Those require explicit producer contracts and separate validation.
Do not copy the upstream mill/metric defaults, XY-only arc handling, fallback
feeds/tool dimensions, omitted travel options or extrusion-to-spindle mapping.
Fusion continues to own CAM paths and compensation; the reviewed plan owns
machine transitions/mappings; LinuxCNC owns configured tools, offsets and motion.

## Pinned sources

- **S1:** [Repository tree](https://github.com/EfrainRodriguez/step-nc-adapters/tree/86a14b8dd3299f46e894a95136e798082febe293), [latest commit](https://github.com/EfrainRodriguez/step-nc-adapters/commit/86a14b8dd3299f46e894a95136e798082febe293).
- **S2:** [Project target and DLL path](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/StepNc.Adapters.csproj), [README](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/README.md).
- **S3:** [Public facade](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/StepNcApi.cs), [models](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/Models.cs).
- **S4:** [Tracked output samples](https://github.com/EfrainRodriguez/step-nc-adapters/tree/86a14b8dd3299f46e894a95136e798082febe293/samples/output), [small test output](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/samples/output/test_program.stpnc#L1-L38).
- **S5:** [APT generator](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AptStepNcGenerator.cs#L40-L96).
- **S6:** [APT tool boundary](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AptParser.cs#L175-L204), [command dispatch/default skipping](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AptParser.cs#L208-L286), [coordinate defaults](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AptParser.cs#L355-L365).
- **S7:** [JSON parsing, defaults and ordering](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AdditiveJsonParser.cs#L55-L142), [point skipping](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AdditiveJsonParser.cs#L240-L270).
- **S8:** [Travel/deposition dispatch](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AdditiveJsonStepNcGenerator.cs#L73-L103), [flow quantity mapping](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AdditiveJsonStepNcGenerator.cs#L339-L359), [optional call handling](https://github.com/EfrainRodriguez/step-nc-adapters/blob/86a14b8dd3299f46e894a95136e798082febe293/AdditiveJsonStepNcGenerator.cs#L451-L471).
- **T1:** [Target package entry point](https://github.com/markmounteer/LinuxCNCNext-NC/blob/72f0db74704fe61f6c28c3cf4cbcc104a32c2a5e/package.json), [translation pipeline](https://github.com/markmounteer/LinuxCNCNext-NC/blob/72f0db74704fe61f6c28c3cf4cbcc104a32c2a5e/src/translate.js).
- **T2:** [Bounded Part 21 reader](https://github.com/markmounteer/LinuxCNCNext-NC/blob/72f0db74704fe61f6c28c3cf4cbcc104a32c2a5e/vendor/fusion360next-nc/part21.js), [profile validation](https://github.com/markmounteer/LinuxCNCNext-NC/blob/72f0db74704fe61f6c28c3cf4cbcc104a32c2a5e/src/profile.js).
- **T3:** [Shared-reader provenance](https://github.com/markmounteer/LinuxCNCNext-NC/blob/72f0db74704fe61f6c28c3cf4cbcc104a32c2a5e/vendor/fusion360next-nc/PROVENANCE.md), [source reader](https://github.com/markmounteer/Fusion360Next-NC/blob/a6e64622343c41545925ebf6e8dc91e12d320ac1/lib/part21.js).
