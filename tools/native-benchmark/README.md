# Native compiler benchmarks

This development harness measures the actual Rust compiler and independent native
bundle loader. It has no controller, Node runtime, G-code backend or GUI in the
measured process. The JavaScript generator uses the public Fusion producer at
`b31904b3d5ad9d56cc4cc16b8090e768b0e1958d` only to create reproducible synthetic
inputs and independent expected fingerprints. It does not generate physical setups.

From a clean translator checkout and a clean producer checkout at that pin:

```powershell
node tools/native-benchmark/generate.cjs C:/dev/Fusion360Next-NC artifacts/benchmark-inputs
cargo build --release --locked --example native_benchmark
python tools/native-benchmark/run.py artifacts/benchmark-inputs artifacts/benchmark-results --worker target/release/examples/native_benchmark.exe
```

On Linux use the same commands with the producer path and the worker filename
without `.exe`. Python 3.11+ is needed. No third-party Python packages are used.
Both directories must be new; runs never overwrite evidence. The runner refuses
an uncommitted source checkout, records source/toolchain/executable/input hashes,
asks Cargo to verify/rebuild the release worker, and refuses a different binary.
It fails if a phase, semantic check, memory measurement or child cleanup fails.
Retain `started.json` and per-process reports if a run is interrupted. Only a final
`results.json` with `complete:true` represents a completed matrix.

Default inputs are 1,000, 10,000 and 34,000 arcs per mill/lathe job, 1,000-arc
inch variants and 34,000-vertex dense polylines. Coordinates are bounded, circular
endpoints are exact quarter turns, every fourth mill arc is a multi-turn analytic
helix, and large jobs retain periodic dwell/coolant changes. Lathe jobs retain
CSS and dimensioned per-revolution feeds; acceptance by this offline compiler is
not synchronization support in a controller. The generator records byte/entity
counts without raising the compiler's resource limits.

The six small committed fixtures use the same generator with a count of eight:

```powershell
node tools/native-benchmark/generate.cjs C:/dev/Fusion360Next-NC NEW_FIXTURE_DIRECTORY 8
```

CI runs these fixtures on Windows and Linux as a harness/semantic smoke check.
It has no performance threshold and does not replace the larger-job measurements.

## Measurement definitions

Each case/phase gets three fresh processes. Each process performs one first call
and five warm calls by default. `prepare`, `compile` and `load` are measured in
separate processes so one phase does not inherit another phase's peak memory.

- `prepare` is complete source/setup parsing, semantic preparation and the
  independent ordered command audit.
- `compile` includes preparation, snapshot checks, encoding and a full independent
  bundle reload/audit. It overlaps the other phases; their timings are not added.
- `load` revalidates embedded inputs and serialized commands with the current
  compiler/schema/policy identities. It never restores permission or live state.
- Core wall time excludes file reads, buffer cloning, result checking/destruction
  and JSON output. Fresh-process first calls do **not** imply a cold OS disk cache.
- Process-start-to-result time includes startup, input reads, **all** calls,
  result checks and report output. It is not a one-job end-to-end CLI latency.
- OS peak RSS includes input buffers, result checks, allocator retention and
  repeated calls. On Windows the harness also records peak private commit;
  Linux records peak virtual space, which is a different metric. Neither is
  live allocator use, a real-time deadline or a Raspberry Pi measurement.

Every call checks the independent producer fingerprint, motion/line/arc/helix/
multi-turn/dwell counts and lack of execution permission. A digest over **all**
ordered commands and provenance spans must agree across repeated calls, fresh
processes and phases. Artifact bytes must agree across compile/create/load.
The process stays alive briefly for the parent to query its OS peak-memory
counter; the timed compiler phase has already finished. This is offline
correctness/resource evidence, not machining-time or continuous path-error proof.
