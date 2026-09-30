# Rust native compiler implementation

Work in progress for stages 1 and 2 of the controller's native execution plan.
The existing JavaScript translator is retained as a revision-1 migration oracle
and G-code tool. The Rust executable has no Node subprocess/runtime dependency.
The single `motion-command` source is the `crates/motion-command` workspace crate.
The controller imports that package by an exact public Git revision and forwards
its types through its workspace facade. Public builds require no private
controller credentials; there is no second set of motion definitions.

Current commands:

```text
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- check-syntax examples/synthetic.stpnc
cargo run --locked -- inspect input.stpnc
cargo run --locked -- preflight input.stpnc plan.json --tool-table tool.tbl --target capabilities.json
cargo run --locked -- prepare input.stpnc plan.json --tool-table tool.tbl --target capabilities.json
cargo run --locked -- publish input.stpnc plan.json --store job-store --tool-table tool.tbl --target capabilities.json
cargo run --locked -- verify-bundle job-store/objects/CONTENT-SHA256.nncb
```

`check-syntax` validates the bounded Part 21 envelope, syntax, references and
accepted entity/attribute shapes. Its report explicitly says `executable:false`.
`inspect` additionally decodes and validates the entire supported source profile.
`preflight` also validates the reviewed setup plan and optionally a tool-table
snapshot and offline target manifest. Both report `executable:false`; neither
publishes native commands or claims command-audit or live-machine qualification.
`prepare` additionally builds and audits the complete unoptimized native plan in
memory, then reports counts and requirements. It also reports `executable:false`;
no native bundle, cache, selected job or execution stream is published. All
optional checks finish before success JSON is printed. A failure prints only
diagnostic JSON on stderr and exits nonzero, including a late tool-table or
capability failure after compilation.

`publish` additionally serializes the complete plan, independently reloads and
audits it, then publishes it in an empty or existing native artifact store. Inputs
must be outside that store. `verify-bundle` checks an existing artifact against
the current compiler/schema/policy and its embedded source/setup. Both report
`executable:false`. The CLI exits without retaining any selection; the future
task adapter must own a fresh selection and live binding. See
[bundle publication and recovery](native-publication.md).

Implemented modules include full revision-1/revision-2 source decoding,
dimensional units, closed executable graphs and ordered source uses, independent
analytic geometry, strict bounded JSON, setup-plan schemas 1–4, tool-table file
checks and offline capability manifests. Exact ordered models and fingerprints
match four legacy fixtures and all 130 captured native profiles. Decimal parsing
and fingerprint formatting retain ECMAScript round trips; derived native radius
does not participate in job identity. See the [capability matrix](native-capability-matrix.md)
and [validation migration checkpoint](native-validation-migration.md).

The profile-shape grammar is generated development data from the pinned legacy
validator, embedded in the binary. Node is used only to capture migration fixtures
with `node scripts/export-native-baseline.js`; it is not invoked at build or run
time. The fixture manifest records all 31 source identities. Its four mill/lathe,
mm/inch cases preserve source text, decoded model, reviewed synthetic plan, tool
table, independent reference G-code and the legacy audited command/source map.
Synthetic transition coordinates are not physical clearances.

## Required work still open

- Explicit fit/blend budgets, exact reductions and measured rate diagnostics.
- Complete check-by-check migration disposition, full negative corpus, native
  geometry/event equivalence and cold/warm/memory/large-job benchmarks.

No native task adapter, controller connection or execution capability is provided
by this checkpoint. Compiler qualification must finish before stage 3 begins.

## Unoptimized command preparation

`compiled::prepare` decodes the source internally, validates the reviewed plan,
builds every command, then calls a separate command audit before constructing the
immutable result. Callers cannot supply an edited inspection model as trusted
compiler input. The plan exposes read-only command/span/source/setup accessors.
Each motion uses the canonical shared revision-2 type; process events use the
shared contract. No geometry/feed/spindle types are forked.

Task-owned resets, coordinate mapping and reviewed single-axis rapid waypoints
are explicit preparation actions. A partial waypoint has an unknown starting
position until live task binding; it is never filled with invented zeroes.
Tool changes invalidate assumed process state, require configured procedure
completion, and are followed by explicit resets before the approach. Reversals
retain their stop event; coolant switches retain off-before-on. Program shutdown
retains stopped spindle, coolant-off, demand reset, reviewed retract, offset
cancellation, feed-context restoration and completion in that order.

Motion feeds are mm/s or mm/revolution; CSS is mm/s with its RPM ceiling.
No nominal RPM conversion is permitted. The first nonrapid motion following a
spindle demand event carries an explicit at-speed gate; intervening rapid moves
and dwells do not consume it. This is a conservative preparation policy whose
controller timing/handshake remains to be qualified in Stage 3. `prepare` checks
compiler-added `at-speed` and setup `linear` requirements as well as source
capabilities. An offline manifest does not constitute live negotiation.

All source segments, including duplicates/zero-length uses, remain ordered.
Revision-1 arcs preserve the original relative-center behavior, including the
commanded start/end of full circles despite reconstruction roundoff. Revision-2
arcs retain analytic sweep/rise/plane and in-plane centers. As in the legacy
translator, actual commanded position supplies the next segment's start; the
profile's numeric continuity allowance never creates an extra positioning move.
The circular axis point is at that commanded start's normal coordinate. CAM
tolerance remains provenance, and allocates no fitting or blending allowance;
termination remains exact path for this baseline.

The separate audit checks the complete span sequence, all source vertices,
reviewed moves, feeds, geometry, intent, gates, state events, tool/mapping order
and completion. Geometry receives an independent continuous-path check in mm.
Output-command limits include setup and state expansion, and are enforced before
construction returns. Spans intern repeated section/path metadata; command records
contain only typed actions and compact use tags. Provenance lookup is logarithmic
in span count and copies at most the relevant source records and two polyline
vertices, never a whole operation/polyline or per-command modal snapshot.

`tests-rust/compiled.rs` compares full ordered physical intent against all four
frozen legacy command/source maps. Coordinates, feeds and event order compare
exactly after dimensional conversion; only CSS allows a few floating-point ULPs
for the legacy intermediate metres/feet conversion. All 130 native fixtures also
prepare without replacing analytic curves with segments. Deleting/replacing each
command (with adjusted spans), changing provenance or motion semantics, reducing
the output bound and omitting compiler-required capabilities all fail closed.
This is compiler evidence; physical tool completion and controller execution
remain unqualified. Offline bundle publication has separate failure tests.

Every native CLI command saves [persistent diagnostics](native-diagnostics.md)
with exact observed input identities, actual stage outcomes, correction guidance
and bounded source excerpts. The standard directory holds `latest-error.txt` and
JSON evidence; later success preserves the latest failure. Archiving failures are
reported separately and never conceal the original preparation error.
