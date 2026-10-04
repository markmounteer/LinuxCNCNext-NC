# Nominal command demand and measured reference diagnostics

```text
nextnc-native analyze-rate job.nncb admission-simulation.json
```

This Rust command independently reloads and audits the whole bundle, checks the
complete measurement summary, then reports command demand. It changes no job or
selection and always reports `executable:false`. The result binds the exact
artifact SHA-256 and measurement-file SHA-256. JSON/TXT diagnostics go to the
[standard archive](native-diagnostics.md), including failed measurement validation.
The library entry points are `rate::Reference::parse` and `rate::describe`.

## Demand is counted after expansion

The report counts every prepared action: motions, reviewed partial-axis waypoints
and state actions, including resets, spindle/coolant changes, tools, offsets,
dwells and completion. Their sum must equal the prepared command count. It also
counts source geometry paths. A single polyline with 34,000 segments therefore
contributes 34,000 motions; it is not treated as one inexpensive source entity.
An analytic circle or helix stays one motion command, including multiple turns.
No diagnostic optimization removes zero-length or repeated source uses.

Known nominal feed time is analytic path length divided by programmed mm/s.
Circular/helical length includes the complete sweep and signed axial rise:
`sqrt((radius * sweep)^2 + rise^2)`. Explicit dwells add their stated duration.
The same formula uses canonical millimetres for mill XYZ, lathe XZ and inch input.

The densest 64-command sliding window is reported, with first/last command
indexes and bounded source-use provenance. Shorter finite runs are considered
at their boundary. This is a fixed diagnostic window, **not** the planner's queue
size. The scan stores at most 64 timing items and one peak, rather than a report
row or cloned modal snapshot for every command.

Rapid moves, unbound reviewed waypoints and per-revolution feeds have unknown
time and split windows. G95/CSS is never converted using nominal RPM. Numeric
overflow/underflow and zero-length feed uses are reported explicitly; no infinite
rate or fabricated duration is serialized. The reported known-duration sum is
only the covered feed/dwell portion, not total job or machining time.

State actions contribute to the numerator. Their controller/procedure waits and
at-speed delays are not estimated in the denominator; neither are acceleration,
jerk or shaping. Nominal demand therefore describes ideal instruction density,
not predicted physical execution time. An at-speed gate remains on the original
motion and is counted; diagnostics never remove it or assume the spindle is ready.

## Reference evidence and comparison

The accepted input is a complete `nextnc-stage0/admission-simulation/1` summary.
It must contain all 21 distinct repeated fixtures, successful completion and
cleanup, disabled/inactive shaping, consistent endpoint checks, source/component/
instrumentation identities, matching trace hashes and passing recorded audits.
The Rust reader checks motion/state counts, observation-window arithmetic,
service-distribution consistency and the reported rate against its raw counts
and elapsed nanoseconds. Input is capped at 1 MiB, with bounded nesting/values
and duplicate-key rejection. A failed or incomplete matrix produces no successful
analysis output. The original input and failing case are identified in diagnostics.

The preserved [reference fixture](../tests-rust/fixtures/admission/README.md)
comes from the qualified instrumented custom-stack run. Its raw geometry and
mailbox audits were performed by the separately pinned Rust simulator harness.
This reader validates the saved summary; it does not authenticate the author or
rerun those raw trace audits. All observation/binary hashes remain in the output.
The measurement is advisory evidence, never a source of execution permissions.

Observed delivery and service-call time stay separate. Reference delivery includes
producer/planner delays and inter-call logging. Thus the slow multi-turn-helix
workload cannot be interpreted as a universal arc-ingress limit. The report
retains all workload samples and the observed all-call range for the job's machine.

`NOMINAL_DENSITY_ABOVE_REFERENCE` means the nominal prepared-action density is
higher than every recorded reference all-call rate for that machine. This is a
screening comparison between **two explicitly different domains**, not utilization,
a saturation threshold or proof of starvation. State actions may lower to zero,
one or several mailbox calls; native adapter expansion and downstream planner
piece expansion remain unqualified and are returned as null. That qualification
belongs to Stage 3's executing adapter. Compiler polyline expansion is already
counted exactly in this report.

No native capacity or universal commands-per-second default is invented. Calling
the library without measurements reports demand with no comparison threshold.
Low or absent warnings do not establish sufficient admission or feasible motion.
Use dense-window provenance to inspect the source, preserve available analytic
geometry, and qualify the eventual adapter with matched workloads before drawing
motion-performance conclusions. This command never changes feeds, geometry,
tolerances or process events to make the comparison pass.

## Verification

Tests bind the complete reference fixture, reject contradictory/incomplete
matrices and duplicate keys, and check exact polyline/state accounting. Independent
closed-form examples check quarter-circle and multi-turn helical length in mm and
inch. G95 fixtures retain counts with unknown duration. Window tests include state
actions and dwell time, and verify that unknown motion durations cannot borrow
time across a boundary. Standalone CLI tests clear PATH, bind the exact artifact
and measurement, preserve the artifact, archive successful reports and attribute
malformed reference JSON to the reference file rather than embedded STEP data.
