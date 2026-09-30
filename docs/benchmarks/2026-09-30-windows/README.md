# Windows large-job compiler qualification

The [baseline report](baseline.md) and [complete raw results](baseline.json)
record translator revision `40e15ad00363925402f423221ba4cd4b6704e614`, Rust 1.96.1,
release optimization and the exact worker/input identities. The full matrix
completed: ten synthetic jobs, three phases, three fresh processes per phase,
one first and five warm calls per process: **90 processes and 540 timed calls**.
Ten preliminary bundle creations are additional untimed setup work.

Both 34,000-arc jobs fit the unchanged 32 MiB/500,000-entity limits: roughly
28.6 MB and 476,000 entities each. The mill job retains 8,500 multi-turn helices;
the lathe retains planar XZ circles, CSS and per-revolution feed intent. Dense
34,000-segment polylines and inch variants are separate cases. All measured
results preserve expected fingerprints, primitive counts, complete ordered
commands/spans and deterministic artifacts. No native controller executes here.

Reproduce the inputs and measurements using the [harness instructions](../../../tools/native-benchmark/README.md)
with counts `1000,10000,34000`. Input bytes/hashes, setup hashes, generator/producer
pins, every timing sample, peak-memory counters and compiler/artifact identities
are retained in the raw results. Large source and bundle files remain generated
artifacts; they can be reproduced from the pinned public producer and generator.
Local originals are in the ignored `artifacts/benchmark-inputs-34000` and
`artifacts/benchmark-windows-20260930` directories of the development checkout.

The baseline exposed high peak memory (up to about 2.21 GiB). The matched
[follow-up comparison](comparison.md) and [raw results](streamed.json) measure
revision `9a518ab2f12b15319ad593c205ed199c8d267d97`, which removes full-model/
full-JSON fingerprint temporaries and a duplicate provenance tree. Another
90 processes and 540 timed calls completed against the identical input manifest.
The verifier confirms identical complete ordered commands and source spans.
For the two 34,000-arc jobs, median warm preparation takes about 27% less time
and peak preparation RSS falls about 37% (to 1,334 and 1,376 MiB). Bundle compile
and reload also use less memory; the comparison retains every case and phase.
These are observed workstation results, not a guarantee on another host.
The follow-up originals are in `artifacts/benchmark-windows-streamed-20260930`.

The host is a general Windows workstation with uncontrolled OS caches and
ordinary background activity. No CPU affinity, real-time deadline, Raspberry Pi
or machining-speed claim follows. Linux/Windows CI smoke runs qualify the
measurement harness and small fixtures separately from this large-job matrix.
The baseline's 11 CI jobs passed in
[run 36688689649](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36688689649).
