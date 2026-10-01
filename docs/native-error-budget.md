# Native path-control error accounting

`prepare`, `publish` and `verify-bundle` include an `errorBudget` report and archive
it in the ordinary native diagnostics. The report covers every source operation
and inspects all prepared motions. It does not change their geometry, feed, event
order or termination. A prepared candidate still grants no execution permission.

Plans using schemas 1–4 retain exact-path termination and zero additional
fit/blend allocation. Native execution-plan/5 requires an explicit `pathControl`
entry for every operation: `exactPath`, `exactStop`, or `blend` with a positive
`additionalDeviation` in the plan's units. Source CAM tolerance is reported
separately with its provenance; it never grants an execution allowance. Inch
inputs are converted once, and lathe X remains radius. Older sources explicitly
report missing CAM tolerance.

Example path-control entries (the array must match the job's operation count):

```json
"pathControl": [
  {"mode": "exactPath"},
  {"mode": "exactStop"},
  {"mode": "blend", "additionalDeviation": 0.01}
]
```

This is additional reviewed execution intent, not a second use of Fusion's CAM
tolerance. Rapid source paths and reviewed setup waypoints retain exact-path
control. The Rust owner emits termination changes only when required and places
a drain barrier before a changed policy can affect pending geometry. Full-stack
pending/held boundary qualification is still in progress.

The version-2 error report exposes the allocation and selected control per
operation. Positive blend requests report `blend_used_mm: null`: preparation
has not measured the executed continuous path. Exact controls retain zero.
Fitting remains disabled. A prepared candidate grants no execution permission;
native Stage 3 and physical-machine acceptance remain incomplete.

The intended accounting, only when compatible independently established bounds
are available, is:

```text
E_pre_shaper <= E_CAM + E_post + E_fit + E_blend + E_numeric
```

The report does not invent the missing terms:

- A Fusion or source-declared tolerance is metadata; this compiler has not
  compared the toolpath to the original CAD surface.
- Supported source profiles do not carry a complete post-approximation bound.
  In particular, a post log describing a linearized radius-mismatch arc is not
  an error bound embedded in the file. `post_error_bound_mm` remains `null`.
- The largest observed arc endpoint residual is a numeric consistency metric.
  It is not an independently established continuous path-error bound, and the
  validator's acceptance floor is not a budget. `numeric_error_bound_mm` remains
  `null`, even when the observed residual is zero.
- `pre_shaper_total_bound_mm` remains `null`; adding a known CAM tolerance to
  zero fit/blend use cannot establish a total when other terms are unknown.

Downstream shaping, kinematics, tracking, tool deflection and mechanical accuracy
are separate. Neither this accounting nor an endpoint agreement proves finished
part accuracy or clearance of the reviewed setup moves.

The reporter and independent command audit reject a termination policy that
differs from its reviewed allocation. Source, reviewed plan, compiler and policy
identities all remain bound to the immutable job. No roughing multiplier, startup
blend setting or missing-field fallback is applied. A requested blend allowance
is not proof of the downstream continuous error bound; that qualification must
precede machine acceptance.
