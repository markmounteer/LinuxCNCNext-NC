# Native baseline error accounting

`prepare`, `publish` and `verify-bundle` include an `errorBudget` report and archive
it in the ordinary native diagnostics. The report covers every source operation
and inspects all prepared motions. It does not change their geometry, feed, event
order or termination. A prepared candidate still grants no execution permission.

The current compiler uses exact-path termination and applies no curve fitting or
blending. Allocated and used fit/blend allowances are therefore **zero**. Source
CAM tolerance is reported in millimetres with its provenance; it never grants an
additional execution allowance. Inch inputs are converted once, and lathe X
remains radius. Older sources explicitly report missing CAM tolerance.

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

The baseline reporter rejects a different termination policy instead of silently
reusing zero accounting. A future positive fit/blend policy must bind explicit
approved per-machine/job allowances, retain provenance and independently prove
continuous deviation before it can be enabled. No roughing multiplier, startup
blend setting or missing-field fallback is applied here. Fitting remains disabled.
