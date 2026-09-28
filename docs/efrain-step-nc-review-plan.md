# EfrainRodriguez/step-nc: review and improvement plan

Status: all three steps implemented for v0.10.0 on 2026-09-28. See [profile validation](profile-validation.md), the [isolated parser harness](../tools/step-conformance/README.md), and [validation evidence](validation.md). The pinned v0.9.0 review and original plan follow; its research probes ran separately from the translator checkout.

## Recommendation

Use [EfrainRodriguez/step-nc](https://github.com/EfrainRodriguez/step-nc) for independent parsing and schema research. Its separation of Part 21 syntax, EXPRESS definitions, instance loading and model validation is useful. It does not provide a LinuxCNC motion backend. [S1]

Implement three bounded improvements in this order:

1. Complete the field-type checks for our existing accepted profiles, with precise record/attribute diagnostics.
2. Add an independent Part 21 comparison harness to development/CI, using a pinned upstream parser.
3. Report which validation stages actually ran, keeping profile acceptance separate from full EXPRESS conformance.

Retain the dependency-free production translator for **XZ lathes and fixed-axis XYZ mills**, in mm and inch. Do not put the upstream schema loader on the execution path yet. The probes below found both useful agreement and concrete gaps; importing a large validator would not, by itself, establish conformance.

## Pinned baseline and method

- Upstream `master`: `14148ea1e8bcb692ecf53fa8d03ccafea8b02157`, dated 2026-05-02; tree `780e06273c86f5f7a4024c37e9529b8dcf9cdc38`. GitHub identifies MIT licensing and does not mark it archived. The package layout includes EXPRESS parser/dictionary, Part 21 parser/reader/writer, and runtime instance/validation packages. [S1]
- Target: clean local and remote `main` at `5e7e8e498a85c41aa5caf2ed53d0026cf66c62bd`, version 0.9.0. Rechecked [CI run 36476855499](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36476855499) at that commit: successful. Existing test scope and interpreter evidence are in [validation.md](validation.md); production tests were not rerun for this documentation-only review.
- Captured the complete pinned upstream checkout: 364 tracked files, 6,848,343 bytes. Reviewed relevant parser, schema query, reader, type/attribute, validation and interpreter source, package/build configuration, documentation and selected tests. This was not a line-by-line review of every tracked file.
- Local evidence: `D:\dev\Next-Nc research\github-translators-2026-09-28\evidence\EfrainRodriguez--step-nc\detailed-review\`. It contains Git metadata, file hashes, the clean `source` checkout, research loader/probes and JSON results.
- Probes used Node 24.13.1's native TypeScript transformation and a research-only module resolver to load unchanged upstream source. **No dependency installation, upstream package build, full upstream test suite or published-package verification was performed.** The loader is experimental research infrastructure, not a proposed production dependency.
- Only synthetic fixtures were used. No G-code was executed, no private jobs were published, and no controller or machine configuration changed.

## What the experiments established

### Independent syntax parsing is immediately useful

The upstream Part 21 parser read our four semantic fixtures without diagnostics. A separate adapter projected its AST into record IDs, component names, argument values, references, typed values and ordered aggregates. The complete normalized records matched our reader, not just the counts. This comparison covered the fixtures' ASCII strings; it did not establish complete STEP string-escape support. [T1, S2]

| Fixture | Records in each parser | Normalized records equal |
| --- | ---: | --- |
| Lathe, mm | 319 | Yes |
| Lathe, inch | 322 | Yes |
| XYZ mill, mm | 321 | Yes |
| XYZ mill, inch | 324 | Yes |

The upstream parser returned no diagnostic when another entity record was appended after the file terminator. Our existing reader rejected that input. Therefore the upstream parser's empty diagnostic list cannot be used alone as an acceptance gate. It also intentionally recognizes a broader Part 21 language than our closed profiles. [S2, T2]

### Our profile checks leave a small but real type gap

In each machine/unit fixture, three mutations replaced a string field with `42.`: application-context text, a Cartesian-point label, and a product label. **All 12 mutations were accepted by v0.9.0, with unchanged G-code.** The corresponding schema fields are strings; our current checks enforce entity membership, argument counts and machining semantics without exhaustively checking every metadata field type. [T3, S3]

This is a malformed-input diagnostic gap, not evidence that normally generated jobs have incorrect motion. It is worth fixing because successful profile validation should not silently accept a number where the profile requires a string. It requires no new machining option.

### Schema parsing succeeds, but model validation is not yet a trustworthy verdict

The bundled AP238 AIM schema parsed and built without diagnostics, yielding 846 entities, 171 types and 14 global rules. Its header describes changes through 2011; it is not automatically the authoritative schema edition for our producer's 2007 application-protocol declaration. Pin and document the intended schema before making a standards-conformance claim. [S3]

Loading and validating the same synthetic files against that snapshot produced:

| Fixture | Reader warnings | Model errors | Model warnings |
| --- | ---: | ---: | ---: |
| Lathe, mm | 4 | 144 | 315 |
| Lathe, inch | 4 | 146 | 315 |
| XYZ mill, mm | 4 | 144 | 318 |
| XYZ mill, inch | 4 | 146 | 318 |

These are **observed library diagnostics, not established counts of invalid source records**. Samples include missing unit attributes, unresolved expression identifiers and extra `INSTANCED_FEATURE` parameters. Source inspection reveals reasons to investigate the checker before changing the producer:

- Attribute-slot collection deduplicates by unqualified name. `INSTANCED_FEATURE` inherits distinct `name`/`description` attributes through two branches, but the queried slot list collapses them. Our emitted six arguments are reported against four slots. Preserve declaring-entity identity when analysing this case. [S4]
- Complex-record loading chooses one entity definition, with a last-record fallback, rather than retaining every co-occurring type. Our unit records combine SI/conversion-based and dimensional unit components. Those combinations need explicit verification before trusting downstream type/rule errors. [S5]
- In a minimal schema, supplying a string to an optional REAL attribute produced no reader or model diagnostic: the setter rejected it, its returned diagnostic was discarded, and the optional attribute remained unset. An extra argument produced only a reader warning. [S5, S6]
- A minimal schema containing an always-false global rule produced no model error. `validateModel` calls instance, reference and uniqueness checks but does not evaluate the schema's global rules. Entity WHERE evaluation failures/unknown results are warnings. A zero-error summary is therefore insufficient. [S7]

These limits make an independent syntax comparison practical today, while full model validation remains an experiment. Do not rewrite unit records or remove feature arguments simply to silence these results.

## 1. Complete the accepted-profile field contract

Replace the arity-only declaration in `src/profile.js` with one internal profile-contract description from which argument counts and basic field checks are derived. This is code-owned format metadata, not another user configuration file. Use the pinned EXPRESS declarations to check the description, but independently review the accepted subset instead of generating it blindly through the upstream slot/model APIs.

Cover the entities and complex components already accepted by the translator:

- Required/optional string fields, including empty strings where allowed; preserve existing Unicode handling.
- Primitive values, typed measures, reference-valued fields, enums and aggregate element/cardinality shapes required by our profiles. Retain current finite-number, safe-reference and geometry checks.
- Distinct `$` and `*` semantics. A derived unit slot can use `*` only in the supported component combination that redeclares it; do not confuse omission with an optional null.
- Attribute identity qualified by its declaring entity where inheritance repeats names. Keep complex-unit combinations and independent component attributes explicit.
- Existing profile restrictions remain authoritative: a construct being valid in general AP238 does not make it executable by this translator. Existing geometry, relationship, units, toolpath and plan validators remain responsible for their semantics.

Run these structural checks over parsed records before building property associations or decoded machining operations. On failure, return a stable `PROFILE_SHAPE` diagnostic with record ID, component/entity, qualified attribute, one-based parameter index, expected shape, actual value kind and available record-start line/column. Do not invent an exact parameter column if only the record location is available. Include operation context when it is already established; otherwise identify the record honestly.

The correction should identify a profile/file mismatch and direct the user to inspect the named field or regenerate with the supported producer. Do not recommend changing compensation, feeds or offsets for a metadata type error. Preserve all-or-nothing CLI publication and existing diagnostic archiving.

**Acceptance:** all 12 numeric-metadata probes fail with the right record/attribute, for both machines/units. Add focused tests for optional nulls, derived placeholders, qualified duplicate names, typed measures, supported unit combinations and malformed aggregate members. Every existing valid fixture, source fingerprint, reviewed plan and golden G-code must remain unchanged. Previously accepted malformed fields intentionally become errors; legitimate older profile files must continue to pass.

Candidate files: `src/profile-contract.js`, a small `src/profile-shape.js`, integration in `src/profile.js`, and focused profile-shape/CLI tests. Keep one source of truth for field count and shape rather than maintaining a second independent arity table. No CPS change is necessary for the current correctly typed output.

## 2. Add independent parser comparisons in development/CI

Create an isolated tool under `tools/step-conformance/`, with its own pinned dependency/build lock and MIT notices. Start with `@step-nc/p21-parser`; the schema-aware reader is not needed to compare syntax trees. Use a reproducible build of the reviewed source and record its source/bundle hashes. A matching npm version number alone does not prove the published bytes match this commit. Replace the experimental research loader with an ordinary supported build before adding the tool to CI.

Keep these dependencies out of the production package, filter and normal `npm test` runtime installation. Give the tool a dedicated CI job and bounded process time/memory. Never fetch an unpinned branch or schema while translating a user job.

The comparison should:

- Independently normalize record/component names, exact numeric values, safe IDs/references, nested typed values, `$`/`*`, enum values and ordered aggregates. Preserve duplicates and aggregate order; define component/record ordering explicitly.
- Verify complete input consumption and the permitted envelope, not just an empty upstream diagnostic list. Keep existing profile acceptance narrower than generic Part 21 acceptance.
- Implement and test string normalization independently, with hand-specified apostrophe, backslash and `X2` escape expectations. Do not call our parser's decoder to manufacture the second parser's answer.
- Exercise both machine profiles/units, complex units, shared geometry, compacted polylines, repeated vertices, arcs/full circles, dwells, metadata and deliberate record renumbering/format changes.
- Include negative controls for truncation, trailing records, malformed escapes, duplicate/unsafe IDs, dangling references and invalid field shapes. Distinguish syntax-parser limits from the profile checks in step 1; a generic parser is not expected to reject every schema/type error.

Store explicit expectations for the demonstrated upstream gaps at the pinned version. An upstream acceptance where rejection was expected is a recorded discrepancy, not conformance success. Unexpected discrepancies fail the CI comparison; do not suppress every warning or accept any difference through a blanket allowlist.

Emit a machine-readable result with source hashes, engine/build identity, fixture identity, stage, expected/actual result, normalized-record agreement and source spans. Archive it in CI with the synthetic inputs. These records support developer investigation and do not authorize machine execution.

**Acceptance:** the four baseline record comparisons above reproduce in the supported build; positive fixtures match; deliberately altered values/references are detected; each negative control has an explicit stage/outcome; both Windows and Linux are covered. The production install remains dependency-free and existing Node/native LinuxCNC tests continue to pass.

## 3. Make validation coverage explicit

Add an optional versioned inspection result describing the checks the production translator actually performs: bounded Part 21/profile structure, profile geometry/process semantics, execution-plan validation, optional table snapshot, ordered command completeness and final serialization. The shape result introduced in step 1 should have record/component counts and contract version. Keep it outside program fingerprints and plan binding.

Represent full EXPRESS instance/type WHERE, global RULE and other general AP238 conformance as **not checked**, with scope, rather than deriving a green status from the profile checks. Older archives show absent coverage as “Not recorded”. Existing tool-table/controller unknown states remain unchanged; avoid another independent success flag that could disagree with the real validation results.

For the separate conformance tool, report EXPRESS parse/build, instance loading, attribute/reference checks, entity WHERE, type WHERE, uniqueness and global rules separately. Preserve errors, warnings, unsupported/unevaluated rules and discarded-value discrepancies. Bind any schema-dependent results to the schema filename, hash, provenance and intended edition. The bundled snapshot's successful parse is one stage, not the overall result.

The full schema-loader experiment should remain outside production until its prerequisites are met: diagnostic propagation, qualified inherited slots, co-occurring complex types, STEP strings, complete document consumption and auditable rule coverage. Any future fixes must have independent minimal regressions and a new pinned checker baseline. Do not waive the current hundreds of messages or attribute all of them to our files without isolating their causes.

**Acceptance:** JSON and HTML distinguish profile validation from standards conformance; successful profile translation still shows general EXPRESS checks as not checked; old/failure archives render truthfully; unsafe text is escaped; no new machining controls or runtime schema downloads appear. The independent CI artifact preserves every stage's evidence without changing the user's job archives.

Candidate files: additive inspection assembly in `src/profile.js`/`src/translate.js`, `src/report.js`, report/CLI tests, `docs/preflight.md`, `docs/validation.md`, and the isolated tool's result schema.

## Delivery and deferred work

Deliver the field contract first, then the independent parser harness and coverage reporting as reviewable commits. Maintain XZ radius semantics, XYZ fixed-axis semantics, all current arc planes, exact numbers, source provenance and exclusive publication throughout. Run the current Windows/Linux Node matrix and standalone LinuxCNC suite before releasing behavior changes.

No change to machining strategy, controller offsets, tool tables, G61 policy, Fusion compensation or generated geometry follows from this review. No schema-selection property is added to Fusion. ARM feature interpretation, arbitrary third-party AP238 execution, feature-derived cycles, rotary machining and live controller feedback remain separate projects. The upstream writer may eventually help create independent fixtures, but it is not a replacement producer or a geometry oracle in this plan.

## Pinned sources

- **S1:** [Repository](https://github.com/EfrainRodriguez/step-nc/tree/14148ea1e8bcb692ecf53fa8d03ccafea8b02157), [MIT license](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/LICENSE), [package/build configuration](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/package.json).
- **S2:** [Part 21 parser](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/p21-parser/src/parser/parser.ts), [parameter AST conversion](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/p21-parser/src/parser/parameter.ts), [string scanning](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/p21-parser/src/lexer/scanner.ts#L31).
- **S3:** [Bundled schema and history](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/docs/express/APs/ap238_schema.exp#L2), [string label type](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/docs/express/APs/ap238_schema.exp#L526), [application context](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/docs/express/APs/ap238_schema.exp#L1446), [product](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/docs/express/APs/ap238_schema.exp#L11925).
- **S4:** [Unqualified slot collection](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/express-dictionary/src/query/entity-query.ts#L73), [attribute declaring-entity metadata](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/express-dictionary/src/types/attribute.ts#L10), [INSTANCED_FEATURE inheritance](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/docs/express/APs/ap238_schema.exp#L6140).
- **S5:** [Entity selection and loading](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/p21-reader/src/entity-loader.ts#L156), [reader pipeline](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/p21-reader/src/read-p21.ts).
- **S6:** [Attribute setter diagnostics](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/step-factory/src/attributes/attribute-access.ts#L32), [type compatibility](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/step-factory/src/attributes/type-mapping.ts), [reader value conversion](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/p21-reader/src/parameter-converter.ts).
- **S7:** [Model validation entry point](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/step-factory/src/validation/validate-model.ts), [WHERE evaluation outcomes](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/step-factory/src/validation/validate-where-rules.ts), [schema rule storage](https://github.com/EfrainRodriguez/step-nc/blob/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/express-dictionary/src/types/schema.ts).
- **T1:** [Synthetic semantic fixtures](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5e7e8e498a85c41aa5caf2ed53d0026cf66c62bd/test/support/semantic-fixture.js).
- **T2:** [Current bounded Part 21 reader](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5e7e8e498a85c41aa5caf2ed53d0026cf66c62bd/vendor/fusion360next-nc/part21.js).
- **T3:** [Current profile arities and semantic checks](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5e7e8e498a85c41aa5caf2ed53d0026cf66c62bd/src/profile.js), [current feature emission](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5e7e8e498a85c41aa5caf2ed53d0026cf66c62bd/vendor/fusion360next-nc/next-nc.js#L287), [final output verification](https://github.com/markmounteer/LinuxCNCNext-NC/blob/5e7e8e498a85c41aa5caf2ed53d0026cf66c62bd/docs/output-verification.md).
