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
cargo run --locked -- inspect input.stpnc
cargo run --locked -- preflight input.stpnc plan.json --tool-table tool.tbl --target capabilities.json
```

`check-syntax` validates the bounded Part 21 envelope, syntax, references and
accepted entity/attribute shapes. Its report explicitly says `executable:false`.
`inspect` additionally decodes and validates the entire supported source profile.
`preflight` also validates the reviewed setup plan and optionally a tool-table
snapshot and offline target manifest. Both report `executable:false`; neither
publishes native commands or claims command-audit or live-machine qualification.

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

- Native process-state compilation and independent completeness/policy audits,
  including ordered geometry/event/source-map equivalence with the original
  translator. Keep optimization disabled for the first parity gate.
- Immutable bounded prepared plans, explicit fit/blend budgets, exact reductions,
  full provenance and measured rate diagnostics.
- Staged bundles, content identities, selection generations, cache rules and
  failure/crash/cancellation/source-mutation/storage/corruption tests.
- Complete check-by-check migration disposition, full negative corpus, native
  geometry/event equivalence and cold/warm/memory/large-job benchmarks.

No native task adapter, controller connection or execution capability is provided
by this checkpoint. Compiler qualification must finish before stage 3 begins.
