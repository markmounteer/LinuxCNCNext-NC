# Native source-corner budgets

Status: development checkpoint, with twelve unshaped desktop simulator captures.
Full Stage 5 qualification remains open. No controller deployment or physical
machine acceptance is established by this work.

After live binding, Rust lowering derives reduced velocity, acceleration and
jerk limits within each compatible source polyline. It uses the observed
interpolation period from the immutable timing contract. The planner still
generates motion profiles and owns braking, overrides and spindle feedback.
The compiler does not generate servo samples or motor pulses.

Budgets preserve every original motion, coordinate, feed, source identifier and
event. A group ends at rapids, arcs, zero-length moves, path-ordinal resets,
feed/movement/tolerance changes, gates, drains and state/process events. Exact
stop and blend modes are excluded from this algebra. Exactly collinear groups
retain their original limits; a corner elsewhere does not cap unrelated groups.
Requested G94 feed remains separate from the hard geometric velocity ceiling.
G95 retains its feed-per-revolution value and measured-spindle demand.

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

Windows passes 57 task/FFI tests; Linux passes 149 compiler-workspace tests.
Clippy passes on both platforms. Fixture tests cover 0.5, 1 and 2 ms calculations;
the full-stack corner capture qualifies only the saved 1 ms cases. The earlier
0.5 ms controller startup failures remain unqualified. Mathematical correctness
under arbitrary geometry and changing controls is not established by these cases.

Controller-side evidence and reproduction live in
`controller/motion/motion/tests/nextnc-stage5/evidence/native-corner-r1` and
`controller/motion/motion/docs/nextnc-native-stage5-native-corners.md` in the
LinuxCNC development branch. Remaining work includes shaped comparative error,
changing demand and limits, fault recovery, timing and the other comparison lanes.
