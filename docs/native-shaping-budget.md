# Native shaping error budget

`nextnc_task::shaping` implements an off-thread geometric allocation component
for a common positive FIR followed by the pinned rate-one LinuxCNC cubic. Rust
lowering now applies it to planar XY source charts with a positive reviewed
execution allowance. Task ABI 5 requires actual common-FIR
coefficients for an enabled filter, preserves the validated kernel in the bound
plan, and fingerprints its exact data with the observed motion birth/timing.
Changed kernels are refused at start, resume, tool confirmation and suffix
rebind. Full-stack geometric, arithmetic, recovery and timing qualification
remains required; a lowering certificate alone does not complete Stage 5.

The component was motivated by the controller's
[shaped-arc failure](https://github.com/markmounteer/linuxcnc/pull/1506): the raw
analytic path stayed on the source but a commanded joint point after filtering
was more than 0.013736 mm from it, exceeding even the old diagnostic's 0.002 mm
comparison. That fixture's reviewed execution policy is exact-path: its
0.002 mm **CAM tolerance is not additional execution permission**. It now
refuses before a candidate is returned. A positive-allowance experiment must
use a separately reviewed plan and must not be presented as an equivalent-policy
performance comparison with the original. Satisfying velocity, acceleration and
jerk limits alone does not establish geometric accuracy.

## Conditional geometric bound

Let `g(s)` be a continuous source curve parameterized at unit speed over the
entire source portion represented by retained samples. Let regular curvature
be bounded by `K` and the sum of tangent-vector jump norms at corners in that
portion be bounded by `J`. These are geometric bounds, not machine acceleration
limits. A line has zero regular curvature; a constant-radius circle has `1/R`.

For nonnegative normalized weights and source parameters `S`, write `m = E[S]`.
The weighted linear term in a Taylor expansion around `m` cancels. The regular
second-derivative remainder is at most `K Var(S)/2`. Each tangent jump is a
vector multiple of a hinge `(s-c)+`. Its Jensen gap is largest when `c=m`, where
it is `E[|S-m|]/2`, bounded by `sqrt(Var(S))/2`. Summing the jump norms gives:

```
distance(E[g(S)], g(E[S])) <= K Var(S)/2 + J sqrt(Var(S))/2
```

If the source parameter is Lipschitz in time with speed bound `V`, the pairwise
variance identity gives `Var(S) <= V² Var(T)`. Constant speed or constant
acceleration is not assumed. Holds, rest-clamped endpoints and reversals in a
valid continuous curve parameter are covered. A coordinate reset is not a
Lipschitz motion and is not covered. Source traversal and winding remain separate
contracts; this distance bound does not permit reordered motion.

The pinned rate-one cubic is a positive cubic B-spline average of four coarse
points. Its basis at fractional time `u` is:

```
((1-u)^3, 4-6u²+3u³, 1+3u+3u²-3u³, u³) / 6
```

The weights sum to one, have mean sample index `1+u`, and variance `1/3` for
every `u` in `[0,1]`. Composing this basis with the normalized common FIR adds
their independent delay variances. Thus the geometric allocation uses:

```
sigma² = normalized_FIR_delay_variance + period²/3
E_geometry <= K V² sigma²/2 + J V sqrt(sigma²)/2
```

This covers the ideal continuous interpolant between servo knots as well as the
knots. Actual coefficient construction, arithmetic and raw geometric errors
must be enclosed separately; the formula is not a substitute for that evidence.

## Kernel identity and error accounting

The constructor accepts at most 32 strictly positive, finite, ordered, unique
terms with delays below 256 samples and an explicit nonzero period. It never
renormalizes coefficients or changes filter tuning. SHA-256 covers the model,
period and exact binary64 coefficient/delay data. That digest binds data; it
does not prove that this kernel is installed, executing, or machine-qualified.

The variance uses an outward-rounded pairwise sum to avoid cancellation in
`E[T²]-E[T]²`. It bounds the mathematical normalization of the original weights.
The actual weights remain untouched. Their non-unit mass contributes the
separate reserve `|sum(weights)-1| * maximum_raw_position_norm`.

The caller must supply one explicit ledger:

```
upstream_error + arithmetic_error + coefficient_mass_error + E_geometry
    <= reviewed_source_corridor
```

An exhausted, nonfinite or unknown ledger is refused. The constructor's unit
mass gate does not erase the mass error. The allowance refers to distance from
the reviewed source toolpath, not error from CAD. No default or extra tolerance
is inferred from CAM metadata. The independent test example deliberately spends
part of its synthetic allowance upstream to exercise shared-budget accounting;
those test numbers are not production defaults.

Allocation returns the physical ceiling unchanged if it fits. Otherwise a
fixed 64-step search returns a checked lower speed ceiling. Every returned
certificate is rechecked against the total allowance. Numerical domains in
which no positive candidate can be represented are refused rather than widened.
All work is off-thread: at most 496 pair contributions and 64 search iterations
per component allocation. The source-window integration has its own bounds below.

## Source windows and executable lowering

The allowance comes only from schema-5 `pathControl.additionalDeviation`, in
canonical millimetres after unit conversion. CAM tolerance remains untouched.
An exact-path curve or polyline with a nonzero enclosed corner jump is refused
when shaping is enabled. Straight coordinate-axis runs remain exact in the
ideal positive-average model. General diagonal collinearity is conservatively
enclosed; floating-point direction equality is not treated as proof of zero
curvature. Numerical and coefficient-mass errors still need qualification even
for ideal straight motion.

Contiguous arcs with the same center and Z form a circle chart, including feed
changes and direction reversals. The minimum enclosed radius gives curvature;
twice the enclosed radius range reserves raw radius variation and projection
back to a source arc. Lines form arclength charts with outward interval bounds
on segment length, normalized tangent and tangent jumps. Unsupported or
unrepresentable domains refuse; square-root bounds use elementary operations,
not an assumed accuracy bound for `hypot`.

The retained history span is bounded by `V * (last_delay + 3) * period` plus
the positional reserve. A two-pointer scan bounds corner jumps only within
that span. A fixed 64-step search couples the span to the proposed speed, so
corners outside retained history do not penalize the whole job. Each predicate
currently calls the component allocator (at most 64 inner iterations); overall
work is bounded by 64 linear scans and fixed scalar searches per chart, with
linear off-thread storage. No new work or allocation is added to the servo.

The approved allowance is spent once, on downstream shaping. Executable
termination is exact-path within those charts, disabling a separate planner
fit/blend expenditure. Original bound source records retain their reviewed
termination, geometry, feed and provenance. The task layout accepts this one
mode rewrite only when the immutable certificate matches the source allowance
and bound kernel. Existing dynamic ceilings and numerical headroom still apply;
the shaping cap can only lower them.

Chart, allowance, lane and discontinuity transfers require a real owner drain
of planner, filter and cubic history before the next chart. Same-circle arc
boundaries do not add per-arc drains. State events retain existing lifecycle
barriers. The pending recovery qualification must verify these conditions
under abort, hold, restart and changing controls, not only ordinary completion.

## Verification

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p nextnc-task --example shaping_budget
```

Eight new component tests include direct filter and Hermite evaluation on
changing-speed circles, holds, reversals and corners. They reproduce excess
error at the old arc speed and bound the reduced-speed ideal path without
widening the allowance. Other tests cover exact identity, coefficient mass,
pure delay, exhausted ledgers and invalid kernels. Straight motion retains its
original ceiling. These are component tests, not corrected simulator captures.

The controller-side independent audit verifies 144 Rust allocations using
100-digit Decimal arithmetic and exact rational cubic moment identities. It
rejects altered kernels, missing cubic variance, increased speed, understated
error, missing history and promotion of component results to live qualification.

## Required integration

The ABI 5 snapshot appends a 400-byte kernel record (1,648 bytes total). Model 1
means a common positive XY FIR, axis mask 3, at the observed interpolation
period. It carries up to 32 exact binary64 weights and sample delays; unused
slots are canonical zero. Disabled shaping must supply an empty record. Old
snapshot versions are rejected before their body is read. This deliberately
requires the matching task host and engine to be rebuilt together.

Binding also requires explicit kernel evidence through the safe Rust API.
Lowering checks its period and applies the geometric speed cap above.
Kernel identity establishes numerical configuration, not machine approval.

1. Publish actual immutable runtime kernel coefficients, period and model from
   the motion-owned filter; bind them to the motion instance/birth. Keep machine
   authorization distinct from numeric kernel identity. Unknown data must fail.
2. Carry that evidence through a versioned task snapshot and its fingerprint.
   Recheck it at selection, start, resume and during execution. Retain the
   existing XY/pure-Z and spindle compatibility restrictions.
3. Construct source windows that include every retained FIR/cubic sample. Bound
   curvature, corner jumps, source allowance and coordinate scale in each window.
   A new cap cannot retroactively bound faster retained samples: propagate caps
   across overlapping windows or establish an actual drain before transfer.
   Source labels alone do not establish a geometric or history boundary.
4. Apply the certified ceiling during Rust lowering, preserving source commands,
   hard dynamic limits, feedback/override behavior and event ordering. Account
   for upstream fitting/blending and arithmetic explicitly. No evidence here
   justifies disabling shaping, changing coefficients or loosening a tolerance.
5. Rebuild and independently qualify the exact compiler/task/motion set against
   the retained failure and changing-demand, fault, lifecycle and timing cases.
   Verify whole output curves, not only endpoint or sampled error. Preserve
   reference guard failures until a separate correction is verified.

Kernel publication and ABI transport have separate retained simulator evidence.
Source-window construction and payload lowering are implemented and have Rust
integration tests. Steps 3–5 still require independent full-stack qualification,
including continuous output curves and the numerical operation domain. The
existing 256-epsilon positional reserve is explicit headroom, not a universal
proof of all profile/filter arithmetic. This work does not complete Stage 5.
