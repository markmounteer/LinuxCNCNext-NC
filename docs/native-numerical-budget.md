# Numerical headroom (task ABI 4)

Live-bound lowering now reduces planning ceilings by an explicit floating-point
allowance, while preserving the original geometry, feed intent, termination and
event order. This addresses the observed downstream cubic roundoff at saturated
jerk. It is part of the native Rust path; G-code translation is unchanged.

`NumericalBudget` records the observed interval, coordinate and accumulated
scalar-distance bounds, and the position/velocity/acceleration/jerk reserves.
The positional reserve is 256 binary64 epsilons times the larger bound (minimum
1 mm). Derivative reserves are 2E/dt, 4E/dt² and 8E/dt³. The code rounds reserves
up and remaining limits down, refusing nonfinite or exhausted limits. This
allowance does not certify every possible geometry, shaping or recovery case;
independent continuous-runtime qualification remains required.

ABI 4 adds trajectory V/A/J at snapshot offset 1,216 and scalar origin at 1,240;
the snapshot is 1,248 bytes. The timing record remains at offset 1,176. The host
must provide actual observations, including finite positive trajectory ceilings.
Old headers are refused before the extended body is read. Trajectory ceilings
join the configuration fingerprint; scalar origin joins the initial fingerprint,
like commanded pose. Tool-procedure rebinding recomputes the reserve.

Planning respects both reduced directional axis limits and reduced trajectory
limits. Corner budgets use those remaining limits. Requested G94 and G95 feed
values are retained independently of the resulting hard velocity ceiling.

Analytic circles and helices additionally use [coupled curve dynamics](native-curve-budgets.md)
to account for scalar and curvature acceleration/jerk together, including braking.
The following counts describe the earlier numerical-reserve checkpoint; the
linked curve checkpoint carries its own frozen source and runtime evidence.

Windows workspace tests pass 152 cases; Linux passes 153 including its additional
platform-specific case. Clippy and C/C++ layout checks pass. The companion
controller verifies 52 NML representation cases and 12 full-stack unshaped jobs,
with independent Windows/Linux audits of 95,668 continuous software intervals.
All six native cutting acceleration/jerk bounds pass the original physical
limits; three prior straight-cut bounds were inconclusive. Reference violations
remain retained. Stage 5 and physical machine qualification remain incomplete.

Exact controller-side identities, raw evidence and reproduction are in
`controller/motion/motion/docs/nextnc-native-stage5-numerical-budget.md` and
`tests/nextnc-stage5/evidence/numerical-budget-r1` in the LinuxCNC motion workspace.
