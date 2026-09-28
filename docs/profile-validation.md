# Profile field validation and coverage (v0.10.0)

Both supported Next-NC profiles (XZ lathe and fixed-axis XYZ mill) now run a
field-shape contract before property association or semantic decoding. The
contract is specific to this translator's accepted profile. It does not claim
to be a general EXPRESS schema or accept arbitrary AP238 files.

`src/profile-contract.js` is the single source of field order and arity. It
describes strings, optional `$` values, references and allowed target types,
typed measures, numbers, enumerations and bounded aggregate shapes. It names
each inherited attribute explicitly, including both name/description pairs in
`INSTANCED_FEATURE`. Supported complex unit/context component combinations
are explicit. `NAMED_UNIT.dimensions` requires `*` for supported SI complexes
and a dimensional-exponents reference for conversion units. Ordinary required
fields cannot be replaced by `$` or `*`.

Empty strings remain valid string values. Existing geometry/process validators
still determine supported semantics, positive quantities, unit combinations,
sequence order and meaningful geometry. The contract does not replace them.
Valid emitted programs, fingerprints and reviewed plans retain their behavior;
malformed metadata previously ignored can now fail before output.

## Error detail

`PROFILE_SHAPE` records `record`, `component`, qualified `attribute`, one-based
`parameterIndex`, `expected` shape, `actual` kind, and `sourceLine`/`sourceColumn`.
The source location is the **record start**, not an invented parameter location.
Wrong arity identifies the first missing/extra parameter. Unsupported complex
combinations use `<components>` with a null parameter index. Unknown entity
names retain `UNSUPPORTED_ENTITY`. No operation is invented before decoding.

For example, `APPLICATION_CONTEXT(42.)` identifies
`APPLICATION_CONTEXT.application`, parameter 1, expected `string`, actual
`number`. Correct the named field/profile mismatch or regenerate with the
supported producer. Execution-plan edits cannot repair this input error.
Preflight, translation and the filter preserve the failure in the existing
diagnostics directory and publish no candidate output.

## Coverage in JSON and HTML

`inspection.validationCoverage` uses `linuxcnc-next-nc/validation-coverage/1`.
Results are attached immediately after each actual validator returns. On a
failure the active stage is `failed`, completed stages stay `passed`, and
dependent stages remain `not_checked`.

| Stage | Evidence and scope |
|---|---|
| `part21` | Bounded syntax/header/record IDs/reference existence, record count |
| `profileShape` | Contract version, records, components and attribute counts |
| `semantics` | Existing supported geometry, units and process validators |
| `executionPlan` | Existing plan validation and schema |
| `toolTable` | Optional snapshot hash/count, or reason it was not checked |
| `completeness` | Candidate generation and existing ordered audit result |
| `serialization` | Existing final G-code text audit result |
| `expressSchema`, `expressAttributes`, `entityWhere`, `typeWhere`, `uniqueness`, `globalRules`, `fullAP238` | Always `not_checked`; profile validation does not establish these |

`inspect` runs only the first three stages. Preflight computes and checks the
candidate but does not write G-code. Errors before validation starts have no
coverage; archives without the field render “Not recorded”. Later publication
errors likewise do not imply a G-code file was written. Coverage is additive
report data, excluded from fingerprints and execution-plan binding. HTML
escapes all displayed values.

## Independent development check

The [isolated harness](../tools/step-conformance/README.md) builds the reviewed
MIT Part 21 parser from EfrainRodriguez/step-nc and compares complete normalized
trees. It keeps parser syntax, complete input consumption, independent string
decoding, reference checks, profile rejection and tree agreement separate.
It is excluded from normal installation and never runs in the LinuxCNC filter.
No schema loader, download, machine setting or Fusion machining option is added.
