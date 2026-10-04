# Native source-corner budgets

The current compiler first applies [numerical headroom](native-numerical-budget.md)
below observed axis and trajectory limits. The formulas below then use those
reduced ceilings. Numerical values and runtime counts below describe the earlier
corner-budget checkpoint and do not qualify a newer runtime automatically.

Status: development checkpoint, with unshaped and shaped desktop simulator captures.
Full Stage 5 qualification remains open. No controller deployment or physical
machine acceptance is established by this work.

After live binding, Rust lowering derives reduced velocity, acceleration and
jerk limits within each compatible executable line run. It uses the observed
interpolation period from the immutable timing contract. The planner still
generates motion profiles and owns braking, overrides and spindle feedback.
The compiler does not generate servo samples or motor pulses.

Budgets preserve every original motion, coordinate, feed, source identifier and
event. Source polyline boundaries, restarted or absent ordinals, descriptive
movement labels and CAM tolerance metadata do not end a group: those fields do
not request a stop in the planner. A group ends at rapids, arcs, zero-length
moves, feed changes, gates, drains, disconnected geometry and state/process events. A group
also ends when the adjacent direction cosine falls below the qualified planner's
`1 - 1e-6` line-coalescing threshold. This prevents a sharp corner from reducing
otherwise independent straight runs; a pre-fix failing regression is retained.
Exact
stop and blend modes are excluded from this algebra. Exactly collinear groups
retain their original limits; a corner elsewhere does not cap unrelated groups.
Requested G94 feed remains separate from the hard geometric velocity ceiling.
G95 retains its feed-per-revolution value and measured-spindle demand.

The source-partition correction retains the independent audit's exact joined
and split fixtures. Both now produce identical bound actions, lowered pieces,
command ranges, drain flags and limits while preserving their original source
ordinals. The new integration test fails against the old grouping code and
passes after the correction. Additional unit cases cover singletons, missing or
zero ordinals, restarted ordinals and changes in movement/tolerance metadata.
Execution-boundary tests still preserve feed, gate, drain and event boundaries.

The frozen correction passes 159 Windows and 160 Linux workspace tests, with
Clippy on both platforms. Twelve fresh complete simulator jobs cover mill G94,
lathe G94 and lathe G95, each with joined, split, singleton and relabelled source
paths. The corrected commands and limits are identical across partitions;
103,004 actual downstream cubic intervals satisfy the reviewed source corridors
and physical acceleration/jerk ceilings. Cutting jerk is bounded below 500 mm/s³.
The twelve preceding-runtime captures retain 86,343 intervals. They show the
different limits, but do not reproduce the earlier component-level jerk excess;
their different velocity limits also prevent an equivalent-limit speed claim.
The frozen controller evidence is `corner-partition-r1` under
`controller/motion/motion/tests/nextnc-stage5/evidence` in the LinuxCNC repository.
This finite unshaped check does not qualify arbitrary geometry, changing
controls, shaped output, tiny accepted tails, WCET or Stage 5 as a whole.

For maximum unit-direction jump `D`, minimum segment length `L`, observed period
`dt` and conservative configured acceleration/jerk budgets `A,J`:

```text
As = min(A/2, J*dt/(3*D))
Js = J/2
V  = min(existing hard geometric ceiling,
         (A-As)*dt/D,
         (J-Js-0.75*D*As/dt)*dt²/D,
         L/(3*dt))
```

The three synthetic faceted 512-cut cases at 1 ms produce approximately
0.4625000206 mm/s, 50 mm/s² and 500 mm/s³. The independent full-stack audit
certifies continuous software path error below 0.130 nm, against actual frozen
predecessor witnesses above 0.132 nm. These are numerical software-path results,
not claims about physical machine accuracy. Every original motion remains.

Windows passes 58 task/FFI tests; Linux passes 150 compiler-workspace tests.
Clippy passes on both platforms. Fixture tests cover 0.5, 1 and 2 ms calculations;
the full-stack corner capture qualifies only the saved 1 ms cases. The earlier
0.5 ms controller startup failures remain unqualified. Mathematical correctness
under arbitrary geometry and changing controls is not established by these cases.

The revised runtime independently certifies 104,821 unshaped intervals, 17,013
shaped native intervals and 14,463 shaped reference intervals. The reference
requires explicit observed-filter drain waits at rapid/process boundaries; its
original failures remain archived. The shaped native faceted path has a strict
error improvement against an actual historical pre-fault witness. Different
native/reference hard limits prevent treating this as an equal-limit speed claim.

Controller-side evidence and reproduction live in
`controller/motion/motion/tests/nextnc-stage5/evidence/corner-sharp-r1` and
`controller/motion/motion/docs/nextnc-native-stage5-corner-shaping.md` in the
LinuxCNC development branch. Remaining work includes changing demand and limits,
current-runtime broader controls, changing G95/CSS recovery, timing and the other
comparison lanes. A separate current-runtime checkpoint verifies six explicit
task-loss/full-restart/fresh-native-job recovery sequences (mill, lathe and shaped
mill), including stale receipt and unselected-start refusals; it does not qualify
physical recovery or continuous braking.
