# Command-state policy verification (v0.11.0)

Both XZ lathe and fixed-axis XYZ mill jobs, in mm/inch, now undergo a separate
policy audit between ordered completeness and final-text serialization. It is
mandatory for `preflight`, `translate` and the LinuxCNC filter. There is no new
machining option, external runtime or producer/profile change.

## Actual tool changes

After each actual `Tn M6`, the translator reasserts units and common modes,
then emits `M5 $0`, `M9`, `G97 S0 $0`. Temporary-offset clearing, explicit WCS/H
selection and the complete reviewed approach follow. M6 still invalidates
position and modal knowledge; reset commands establish commanded state only.
They do not infer a remap's position or prove that the physical spindle stopped.

Requested spindle/feed/coolant caches are reconciled at this boundary. The
source's requested coolant-off can no longer be suppressed using a pre-M6
request while emitted state remains unknown. First approach state must be
stopped/off/zero RPM, and process state at each path must match the source.
An actual coolant-off request is no longer allowed to match unknown state.

A same-tool retract still reasserts the contract and mappings without M6.
Compatible `continue` and `link` boundaries retain their inherited process state.
Tool and H numbers remain independent. No approach, source vertex, dwell, feed,
CSS conversion/cap or reviewed transition is removed or rounded.

## Independent audit

`src/policy-audit.js` folds structured commands independently. It does not read
the emitter's `modalState`, `stateChange`, request caches or final state.
The decoded program and validated plan supply obligations; completeness has
already verified the operation/path boundaries and ordered waypoint/source uses.
The auditor is not a second G-code generator or general RS274 interpreter.

It requires:

- Initialized modes and stopped/off/zero RPM at startup and every G53 retract
  waypoint, including final retracts.
- Tool change only after the reviewed retract, before approach, with cancelled
  H and the correct selected T. Unknown post-M6 state must be reestablished;
  WCS/H selection and initialization occur before approach.
- Initial operation process state before the first path's state changes, and
  independently reconstructed process/mapping state at each path use.
- A stop between opposite spindle directions. Equal numeric RPM with a changed
  direction still requires the stop; a speed-only change does not.
- Compatible process state throughout checked continue/link transitions.
- End retract before final H cancellation and feed-mode restoration, then one
  terminal M2. A later M2 cannot satisfy earlier retract preparation.

Known state can satisfy an obligation without repeating a command; unknown
state cannot. Coolant tracking also detects simultaneous/unknown flood and mist
when a required M9 is missing. Independent state checks complement the existing
completeness and final-text audits: no one audit's success implies another's.

## Reports and failures

Successful translation records `execution.policy`, schema
`linuxcnc-next-nc/policy-audit/1`, status, scope and actual checked counts:
startup, retract waypoints, tool changes, approach waypoints, operation entries,
path uses, continuations, link waypoints, reversals, end waypoints and program end.
`validationCoverage.stages.policy` records the actual stage result. JSON/HTML
archives without these fields remain unchanged and display "Not recorded".

Failures use `INTERNAL_ERROR` and a stable `POLICY_*` invariant. Context includes
the output line, execution boundary, phase, owning operation/path where present,
the expected obligation, observed independently reconstructed state, and the
reviewed plan pointer for waypoints/mappings. Policy has no fabricated STEP ID.
Completed stages stay passed; serialization remains `not_checked` on policy
failure. Existing diagnostics archiving, empty stdout and exclusive output-file
behavior apply. Report these implementation failures with the archive; changing
CAM geometry or relaxing the plan is not the remedy.

## Regression evidence and limits

The original eight golden hashes are retained. A test strips only the new reset
trio immediately after each M6 initialization, then checks the original hash
and fingerprint. Independent hand-authored programs include the deliberate
resets and cover both machines/units, off/flood/mist, independent T/H/WCS,
same-tool retracts, continue/link, equal-value RPM/direction and G94/G95 changes,
rapid-to-feed, dwell-only process changes, CSS-cap-only changes and equal S
values across RPM/CSS modes. Complete native canonical traces are compared.

Fault injection removes both commands and output text, misplaces preparation,
restarts early, changes boundary ordering and retains misleading emitter state.
The 16 omission cases from the Mastercam review must now all fail policy.
Unsupported orientations, negative/tilted arc normals, helices and compensation
remain rejected; the accepted profile has not expanded.

See [validation](validation.md) for measured test results. General EXPRESS/AP238,
third-party Mastercam exports, installed M6 remaps, controller feedback,
workholding/clearance and physical execution remain outside these offline checks.
