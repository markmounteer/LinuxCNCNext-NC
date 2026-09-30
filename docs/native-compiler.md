# Rust native compiler implementation

Work in progress for stages 1 and 2 of the controller's native execution plan.
The existing JavaScript translator is retained as a revision-1 migration oracle
and G-code tool. The Rust executable has no Node subprocess/runtime dependency.
It depends on the single `motion-command` crate at the exact Git revision in
`Cargo.toml` and `Cargo.lock`; motion types are not copied into this repository.

Current commands:

```text
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- check-syntax examples/synthetic.stpnc
```

`check-syntax` validates the bounded Part 21 envelope, syntax, references and
accepted entity/attribute shapes. Its report explicitly says `executable:false`.
It does not claim full semantic validation, a reviewed execution plan, machine
readiness or native execution. Those gates are still being implemented.

Implemented library foundations include dimensional unit resolution with graph
cycle/depth checks, closed executable properties/relationships and ordered-use
coverage, independent analytic geometry checks using the shared revision-2
contract, and duplicate-key-rejecting bounded JSON. The semantic fingerprint
serializer matches the legacy ECMAScript number spelling, including negative zero
and exponent thresholds; tests match the exact four captured legacy model hashes.

The profile-shape grammar is generated development data from the pinned legacy
validator, embedded in the binary. Node is used only to capture migration fixtures
with `node scripts/export-native-baseline.js`; it is not invoked at build or run
time. The fixture manifest records all 31 source identities. Its four mill/lathe,
mm/inch cases preserve source text, decoded model, reviewed synthetic plan, tool
table, independent reference G-code and the legacy audited command/source map.
Synthetic transition coordinates are not physical clearances.

## Required work still open

- Full revision-1/revision-2 semantic decoding and ordered source maps, including
  actual Autodesk-engine output comparisons with the Rust decoder.
- Execution-plan and tool-table checks, process-state compilation and independent
  completeness/policy audits. Keep optimization disabled for the first parity gate.
- Immutable bounded prepared plans, explicit fit/blend budgets, exact reductions,
  full provenance and measured rate diagnostics.
- Staged bundles, content identities, selection generations, cache rules and
  failure/crash/cancellation/source-mutation/storage/corruption tests.
- Complete check-by-check migration disposition, full negative corpus, native
  geometry/event equivalence and cold/warm/memory/large-job benchmarks.

No native task adapter, controller connection or execution capability is provided
by this checkpoint. Compiler qualification must finish before stage 3 begins.
