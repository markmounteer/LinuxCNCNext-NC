# Independent Part 21 comparison

Development/CI tooling only. The installed translator and LinuxCNC filter do
not depend on this directory. All checked inputs are synthetic.

```sh
npm --prefix tools/step-conformance ci --ignore-scripts --no-audit --no-fund
npm run test:conformance
```

The tool vendors only the parser source, package metadata and MIT license from
[EfrainRodriguez/step-nc at 14148ea1](https://github.com/EfrainRodriguez/step-nc/tree/14148ea1e8bcb692ecf53fa8d03ccafea8b02157/packages/p21-parser).
Copyright and attribution are preserved in `upstream/LICENSE` and the original
files. No upstream source is modified. `source.json` records the exact Git
commit, original paths, Git blob SHA-1s and SHA-256s, verified against the Git
objects when imported. Git attributes preserve these bytes on both systems.

An ordinary `tsc` build compiles the parser to CommonJS ES2022. TypeScript
5.9.3 is exact-pinned in the isolated integrity lockfile. No experimental
loader, root dependency or package lifecycle script is required. The build
verifies source and compiled-file hashes against committed manifests, then
records compiler, configuration, lockfile, source, output and runtime identity.
To update the oracle, review a new commit, verify its source objects, review
and regenerate both manifests, and reestablish every named discrepancy.
Do not simply waive a changed hash or failed case.

## Comparison and controls

The 56 cases comprise 20 valid fixtures and 36 invalid controls, covering lathe
XZ and mill XYZ in mm/inch. Valid cases include all principal mill arc planes,
major/full circles, shared records, repeated polyline vertices, dwells, process
changes, Unicode/surrogate-pair metadata, apostrophes, backslashes, exact numeric
values, renumbered/reversed records, extra whitespace and CRLF input.

Each full normalized tree includes the header, record IDs, component names,
parameters, references, typed parameters, enums, nested aggregates and distinct
`$`/`*`. Records sort by ID and complex components by name. Parameter/aggregate
order and duplicate references are preserved. Numbers use exact JavaScript
Number values, including signed zero; there is no tolerance or rounding.
Integer and real lexical spellings of the same numeric value normalize alike.
This is not an arbitrary-precision decimal oracle.

The oracle adapter decodes raw STEP strings independently; it never calls the
production decoder. Fixed-answer checks exercise that decoder. Syntax success
alone is insufficient: the adapter checks the parsed end offset and permitted
sections, safe/unique IDs, complete string escapes and reference existence.
Fixtures and expectation records are named and bound to the pinned parser:

| Control | Expected behavior |
|---|---|
| Truncation | Upstream syntax diagnostics; target parse rejection |
| Trailing record | Upstream syntax accepts; envelope adapter and target reject |
| Malformed X2 length | Upstream preserves token; independent decoding and target reject |
| Unsafe/duplicate ID | Upstream syntax accepts; normalization and target reject |
| Missing reference | Upstream syntax accepts; reference check and target reject |
| Three numeric metadata substitutions per machine/unit pair | Both syntax trees agree; target field contract rejects `PROFILE_SHAPE` |

No diagnostics are suppressed. Unexpected stage outcomes or unequal normalized
trees fail the job. EXPRESS schema parse/build, instance loading, general
attribute/reference conformance, entity/type WHERE, uniqueness and global RULE
stages are explicitly `not_checked`. The earlier full-schema research probe
is not part of this harness: its diagnostic propagation, inherited-slot and
complex-entity issues must be resolved before it can be a conformance oracle.

## Artifacts and bounds

`artifacts/step-conformance/comparison.json` contains source/build/input hashes,
engine identity, independent adapter/harness and target hashes, all diagnostics,
AST record spans, stage outcomes, named expected discrepancies, normalized tree
hashes and profile coverage. Completed evidence is saved after each case.
Synthetic `.stpnc` inputs and `runner.json` are included in Windows/Linux CI
artifacts. They do not change the user's job diagnostics. The worker has a
60-second wall-clock limit and 512 MiB V8 heap limit; fixtures are bounded to
4 MiB and 10,000 entities. The build has a separate 60-second limit and each
CI job a ten-minute limit. The runtime translator retains its existing limits.
