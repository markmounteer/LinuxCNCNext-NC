# Native validation migration

The [check-by-check inventory](native-validation-inventory.md) records 259 legacy
assertion/error sites, 61 entity arities, 187 attribute constraints and seven
complex-component combinations. Each site has an explicit disposition, complete
legacy expression and Rust implementation/test references. All 32 legacy modules
have an explicit scope disposition. The JSON evidence pins source and reference
hashes; the development verifier detects stale, missing or changed evidence.
This does not claim exhaustive branch coverage or physical-machine qualification.
Exact reductions, error accounting, measured-reference demand diagnostics and
large-job benchmarks are now qualified by the
[Stage 1–2 acceptance review](qualification/stages12/README.md).
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
| Persistent diagnostic archives and correction guidance | Ported to Rust with separate schema, exact observed identities, bounded source excerpts, actual CLI-stage states and JSON/TXT reports | `tests-rust/diagnostics.rs`, all-command CLI/archive-failure tests; [directory and lifecycle](native-diagnostics.md) |
| Original command contracts, ordered completeness and independent policy audit | Ported to the unoptimized plan and independently decoded native bundle | `command_audit.rs`, serialized-command corruption, exhaustive per-command deletion/replacement/use-tag mutations, geometry/feed/gate mutations |
| Full semantic command/source-map parity with optimization disabled | Four preserved complete fixtures pass ordered geometry/event/waypoint/feed comparison; every legacy validation site has an explicit disposition | `tests-rust/compiled.rs` and the check-by-check inventory; every coordinate compares exactly, with separately bounded CSS conversion roundoff |
| Output-command expansion bounds and source lookup | Ported before immutable plan construction; repeated metadata stored in spans | Limit boundary tests, source/provenance lookup for every fixture command |
| Immutable plan publication, artifact/cache identity, selection generations and failure handling | Offline implementation and failure tests pass; task/arm integration remains Stage 3 | `bundle.rs`, `publication.rs`, process-exit/partial-write/source-mutation/corruption/stale-worker tests; no restored selection on owner restart |
| Exact reductions, error budgets, measured rate warnings and large-job benchmarks | Implemented and qualified offline | [Exact reductions](native-exact-reductions.md), [error accounting](native-error-budget.md), [demand diagnostics](native-command-demand.md), [matched benchmarks](benchmarks/2026-09-30-windows/comparison.md); geometric baseline unchanged, native capacity unqualified |
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
- Native `prepare` additionally qualifies the complete unoptimized in-memory
  command plan. Typed feeds/planes replace G-code formatting modes, atomic shared
  spindle events replace mode/start text pairs, and the at-speed gate is explicit
  before the first nonrapid motion after a spindle event. Stage 3 must qualify
  task timing, live state and handshake behavior. `publish` separately creates an
  audited binary candidate; every CLI command still reports `executable:false`.
- Native baseline termination is exact path. Source CAM tolerance grants no
  implicit fit/blend budget. Reviewed partial-axis setup moves remain unresolved
  task intents until live binding; their unknown starting axes are never guessed.
- G-code's decimal magnitude/exponent-expansion and 240-character line limits
  are replaced by bounded typed binary64 records. Finite positive process values
  must remain finite and positive after dimensional conversion; geometry metrics
  must remain finite. Tests cover values outside text-format limits, exact binary
  reload and converted feed overflow/underflow. These representation tests do
  not establish acceptable physical travel, speeds or feeds. Source-profile
  version-specific geometry limits remain in force.
- Source labels remain opaque metadata. A label resembling an active LinuxCNC
  comment or G-code cannot create a native command; a dedicated bundle test
  compares the complete command sequence after such a label change.
- The native parser accepts inter-token whitespace rather than a fixed newline
  spelling of the Part 21 envelope. Required headers/data/end tokens and exact
  input hashes remain checked. Native source/setup reads use the configured input
  budget; tool-table and target-file reads retain their separate 1 MiB bounds.

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

Prior checkpoint validation: 23 compiler tests and 23 shared-contract tests passed (with 130 native profiles, four
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

The subsequent command-compiler checkpoint adds an unoptimized immutable plan,
output expansion limits, bounded provenance lookup and a separate command audit.
Its four complete legacy comparisons cover mill/lathe, mm/inch, continuation,
links, tool/offset/WCS changes, CSS/feed-per-revolution, reversal, coolant, dwell,
arcs and shutdown. All 130 native cases prepare; an additional fixture explicitly
retains duplicate and backtracking vertices. Mutation loops delete/replace each
command and alter its source-use tag; motion feed/gate/termination/intent/endpoint
mutations are rejected. A source-only capability manifest is rejected when it
lacks compiler-added at-speed/linear requirements. The standalone `prepare` CLI
runs with no PATH/Node and emits no success output on a late validation failure.

The bundle/publication checkpoint adds an explicit bounded wire format, automatic
compiler/source/schema/policy identities and independent serialized-command
verification. Four legacy and all 130 native cases traverse encoding/decoding.
The store uses one OS-locked owner, opaque worker tickets, distinct generations
and staged content-addressed publication. It invalidates selection before a
replacement attempt; owner restart never restores one from disk. Unit tests cover
actual subprocess exits at six checkpoints and injected partial-write/storage
failures. CLI tests publish/reuse/verify with no Node and reject failed replacement
without falling back to the old object. Detailed scope and platform limits are
in [native publication](native-publication.md).

Native diagnostics now archive every CLI command under the standard translator
state directory, with a `NEXTNC_DIAGNOSTICS` override. Reports retain original
errors, collected issues, exact observed input identities and honest stage
outcomes. Correctly attributed source/setup/table/manifest excerpts are bounded
and checked against the original read hash. Successful checks preserve the latest
error. Concurrent native writers, busy indexes, input collisions and failed
archiving are tested; a reporting failure never changes the preparation outcome.

The source inventory also exposed a POSIX FIFO gap in the native read path.
`fileio.rs` now checks a regular descriptor opened nonblocking on Unix, bounds
actual bytes and verifies size/modification metadata around each read.
Publication additionally refuses symlinks and independently rereads/rechecks
snapshots. A timeout-bounded Unix CLI regression test rejects FIFOs in source,
setup and artifact roles without a writer. Existing crash/corruption/storage
tests exercise the shared reader as part of publication.

The inventory verifier runs on Linux and Windows in the conformance CI lane.
It is a development tool using a separately pinned parser; the native runtime
remains Rust-only. All 61 legacy entity arities and 187 attributes are also
mutated individually in the Rust corpus test: extra/malformed attributes must
fail at the mutated source record. This supplements the 143 observed legacy
negative cases rather than treating that captured set as exhaustive.
