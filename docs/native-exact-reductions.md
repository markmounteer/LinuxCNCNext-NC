# Exact reductions and their evidence

Stage 2 retains every source motion and ordered process event required by the
unoptimized execution policy. The reductions implemented here change storage and
preparation cost. They do not fit curves, combine geometric segments, discard
duplicate vertices or spend the source CAM tolerance.

| Reduction | Invariant | Evidence |
| --- | --- | --- |
| Intern operation/path metadata in spans | Every ordered source use remains attributable, including repeated vertices | `tests-rust/compiled.rs`: complete ordered legacy comparisons, all native profiles, per-command deletion/replacement and provenance mutation checks |
| Emit process-state changes instead of a full modal snapshot for each motion | Source spindle changes, reversals, coolant off-before-on, tool/offset transitions, dwells and end-state ordering remain explicit | Independent `command_audit` plus full legacy event comparisons; the CSS rounding-collision regression preserves distinct source events even when conversion produces equal floating-point values |
| Stream canonical fingerprint bytes through a fixed buffer | Identical semantic fingerprint to the materialized canonical JSON oracle | All preserved models, numeric/string edge cases and multi-buffer strings in the fingerprint tests |
| Move owned provenance into its envelope | Identical ordered source mappings without a second full tree | Full ordered command/span digests in the matched benchmark and compiler/bundle audits |
| Keep revision-2 circles and helices analytic | Plane, direction, sweep, multiple turns and axial rise stay explicit | Native producer/consumer geometry corpus, independent path checks and 8,500 multi-turn helices in the large mill benchmark |

The first two are part of the unoptimized semantic port. They are not a new
geometric optimizer. In particular, setup/reset actions are not removed merely
because an offline state model suggests they might be redundant: live task state
is not yet bound. Operation changes, at-speed requirements and all reviewed
waypoints retain their existing behavior.

The [matched Windows benchmark](benchmarks/2026-09-30-windows/comparison.md)
compares source revisions `40e15ad00363925402f423221ba4cd4b6704e614` and
`9a518ab2f12b15319ad593c205ed199c8d267d97`. Input identities and complete
ordered command/span digests agree across 540 timed calls in each matrix.
The largest arc-heavy jobs show lower preparation memory and time after the
temporary-allocation reductions. These are workstation measurements, not motion
speedups or real-time deadlines.

The [error-budget report](native-error-budget.md) allocates and uses zero fit and
blend allowance for this policy. Post approximation and continuous numeric bounds
remain unknown where the source supplies no evidence. Endpoint residuals are
consistency checks; they are not a proved continuous error bound.

The controller's Stage 0 direct replay already measures existing planner
coalescing/blending separately. Its saved dense/collinear cases show no reduction
in emitted pieces and no positive-blend speedup. That result does not establish
that all possible paths behave the same way. An upstream segment combiner would
need its own continuous-path and ordered-barrier proofs before it could replace
this baseline; none is introduced here.
