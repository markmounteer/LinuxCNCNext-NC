# Native validation migration checkpoint

This is a disposition of implemented validation groups, not yet the final
check-by-check Stage 2 acceptance inventory. Command generation, independent
audits, publication and optimization remain missing and prevent native admission.
The original JavaScript runtime is retained unchanged as a development oracle.

| Existing checks | Current Rust disposition | Evidence |
| --- | --- | --- |
| Part 21 envelope/header, finite numbers, strings, record identity, references | Ported with bounded parsing | `tests-rust/part21.rs` |
| Accepted entity components, inherited attribute shape, target type and aggregates | Ported from embedded closed shape grammar | `shape.rs`, malformed shape corpus |
| Dimensional units, context, inch/minute conversion, cycles/depth | Ported | `tests-rust/units.rs` |
| Allowed properties/methods/relationship endpoints, representation associations, ambiguity, unique ordered uses, orphan executable definitions | Ported | `profile_graph.rs`, graph tests and negative corpus |
| Geometry, initial/path process state, feeds, speed/cap, coolant, tools/offsets, dwell, continuity | Ported for supported source subset | Complete legacy/native model comparisons in `tests-rust/profile.rs` |
| Reviewed plan schemas 1–4, program binding, units, machine, T/H/WCS mapping, exact continuation, links, ordered single-axis approaches/retracts/end | Ported | `tests-rust/plan.rs`; 54 captured legacy failures with same error codes |
| File tool-table size, syntax, T/P/Q spelling/ranges, duplicates and mapped record presence | Ported | `tests-rust/plan.rs`; 15 captured failures with same error codes |
| Error source identity and ordered geometry/property uses | Ported with UTF-8 byte columns and additional sequence-link records | Profile tests; errors contain section/path context; no source reference participates in semantic fingerprint |
| Original command contracts, ordered completeness, independent policy audit, serialized-output audit | Missing from native command path | Must be implemented before a prepared native plan can be accepted |
| Full semantic command/source-map parity with optimization disabled | Missing | Four legacy fixtures retain full original command/source maps and independent reference G-code for the upcoming comparison |
| Immutable plan publication, artifact/cache identity, selection generations and failure handling | Missing | No native command bundle/cache/selection is published by this checkpoint |
| Exact reductions, error budgets, measured rate warnings and large-job benchmarks | Missing | Geometry is not reduced and no speed claim is made |
| GUI/filter/report visualization | Outside the native compiler request | Existing JavaScript functionality remains available; no native GUI integration claimed |
| Full AP238 EXPRESS/WHERE/global rules, physical clearance, live tool/coordinate/controller binding | Never established by these subset checks | Remains explicitly unverified |

## Intentional differences

- Native syntax rejects unsupported escapes, raw control characters and
  leading-zero entity IDs rather than accepting ambiguous spellings. Valid writer
  output uses supported escapes and canonical IDs. Duplicate JSON keys are
  rejected instead of silently taking the last value.
- Revision 2 adds tolerance/movement/capability fields and analytic helices/multiple
  turns. Revision-1 models and fingerprints remain unchanged. A source radius is
  preserved in revision 1; a derived native radius is omitted from revision-2
  fingerprint identity to avoid cross-runtime math rounding differences.
- Source columns are one-based UTF-8 **byte** columns, explicitly labelled in
  provenance; the older JavaScript source map used UTF-16 string columns.
- Rust profile errors use specific codes instead of wrapping every reader
  rejection as `INVALID_NEXTNC`; the semantic rejection must remain. Setup-plan
  and tool-table corpus tests additionally require the original error codes.
- Native `preflight` currently qualifies only source, setup plan, optional
  tool-table snapshot and optional offline capability manifest. It explicitly
  reports the missing command audits, live binding and physical checks. It must
  not be mistaken for the older translator's complete G-code preflight.

## Captured failure corpus

The 143 committed cases in `tests-rust/fixtures/negative` were observed while the
existing six source/setup validation test files passed (48 tests). Each file
contains the actual rejected input and original diagnostic: 74 source/profile,
54 execution-plan and 15 tool-table failures. File names are SHA-256 digests of
their exact JSON contents. `scripts/capture-native-negative-oracle.js` wraps only
development test calls and preserves original returns/exceptions. It excludes
non-string or over-1-MiB source captures and unexpected programmer exceptions;
therefore this corpus is not evidence that every possible legacy rejection has
been enumerated. Explicit Rust resource tests cover the parser limits separately.

Checkpoint validation: 23 compiler tests and 23 shared-contract tests passed (with 130 native profiles, four
legacy models, 143 negative cases and additional mutation loops inside them);
Clippy with warnings denied passed. The Fusion suite passed 84/84 and generated
CPS verification passed. The retained translator suite passed 104 tests, with
its real POSIX filter test skipped on Windows. That skip does not qualify the
POSIX filter and is unrelated to native execution readiness.

The first hosted native CI attempt failed before compiling because its contract
dependency pointed into the private controller repository. The newly authored
machine-independent contract is now the canonical public workspace crate, and
the controller imports a pinned revision through a facade. Public builds need
no controller credentials. This changes ownership, not motion type semantics.
