# Coupled dynamics for native circles and helices

Native task lowering now budgets Cartesian acceleration and jerk together for
constant-radius circles and helices. Previously, independently bounding scalar
jerk and cruise curvature missed their combined effect during acceleration and
braking. The controller's retained simulator captures demonstrated both vector
and individual-axis jerk violations, including actual planner fault stops.

For path speed `v`, scalar acceleration `a`, scalar jerk `j`, curvature `k` and
torsion `t`, the physical derivative norms obey:

```
acceleration² = a² + (k v²)²
jerk² = (j - k² v³)² + (3 k v a)² + (k t v³)²
```

The allocator bounds the complete box `0 <= v <= V`, `|a| <= A`, `|j| <= J`.
It includes the worst braking sign, so the tangential jerk bound is
`J + k² V³`. It applies the remaining trajectory and in-plane axis ceilings
after numerical headroom, and independently constrains the helix's axial
acceleration and jerk. An absent lathe axis contributes nothing to a planar arc.

At most 272 candidates are evaluated per source arc, entirely outside real time.
The feasible candidate with the shortest estimated symmetric rest-to-rest
S-curve time is selected. This is a bounded allocation heuristic, not a proof of
whole-job optimality or a measured performance improvement. Invalid or
nonrepresentable geometry/limits fail lowering.

Source geometry, analytic sweep, winding, feeds, termination and process order
are preserved. Requested G94 feed remains separate from the hard speed ceiling;
G95 retains feed per revolution and the controller's measured-spindle demand.
The same immutable hard limits apply when the planner changes demand or brakes.
Straight-line lowering and the JavaScript G-code translator are unchanged. No
ABI or bundle-format change is required; the updated execution library is still
identified independently from the prepared bundle's compiler fingerprint.

Five component tests include 480 allocated boxes and 138,240 independently
differentiated Cartesian states across all planes, winding directions, planar
and pitched geometry, asymmetric axes, and acceleration/jerk signs. The frozen
Windows workspace passed 157 tests and Linux passed 158, with Clippy passing on
both platforms.

The companion controller checkpoint records ten corrected analytic fault stops
plus a pilot, and eight complete native jobs paired with eight retained reference
jobs. All eight native full-job cutting bounds certify below the original
100 mm/s² and 1000 mm/s³ physical ceilings. The unchanged reference jobs still
violate jerk; their times are not a comparison under equivalent certified limits.
Raw evidence and independent rerun instructions are in the LinuxCNC repository:
`controller/motion/motion/docs/nextnc-native-stage5-curve-budget.md`.

These results qualify the recorded unshaped constant-radius cases. They do not
establish arbitrary-state stopping, changing machine limits, shaped curves,
explicit fault recovery, geometry outside the constant-radius model, performance
optimality, newer controller execution binaries or physical-machine acceptance.
Stage 5 remains incomplete.
