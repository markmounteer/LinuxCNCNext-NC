# Command and state records

Version 0.7.0 introduced source provenance and command/completeness checks;
v0.9.0 additionally audits the final serialized text. These checks apply to
XZ lathes and fixed-axis XYZ mills, in mm/inch.

## Source provenance

Each source-map entry now has `provenance.origin`: `step`, `execution-plan`, or
`translator-policy`. STEP entries identify the owning workingstep/operation and
toolpath, with one-based source line/column and actual record IDs. Linear moves
identify both source vertices; arcs retain circle/placement/center/end references,
including the derivation of full-circle endpoints. Process commands identify
their representation/property records. The source curve start remains separate
from the commanded start at a tolerance-accepted join.

Plan movements identify a JSON Pointer, for example `/sections/0/retract/0`;
T/H and WCS commands identify their mapping fields. Header/reset/end policy has
no invented STEP record. `inspection.traceability.provenance` holds the complete
source sidecar, including process properties that did not require a new modal
command. It is bound to the parsed input text's SHA-256. CLI archives additionally
retain the exact file-byte input/plan hashes and candidate G-code hash.

This sidecar is outside the decoded model/fingerprint. STEP renumbering and
formatting can change provenance while preserving plan acceptance and G-code.
These are identities within one input, not permanent Fusion IDs. The HTML report
displays provenance with operation commands and in the full archive.

## Command and completion checks

The internal emitter validates command fields, enum/numeric values, machine axes,
units, work offsets, tool/H IDs, frames, arc planes and lifecycle before mutation.
It rejects commands after M2, invalid axes and arc plane/modal mismatches. M6 can
invalidate state; the existing explicit initialization sequence reestablishes it.
Full circles retain empty endpoint words and in-plane center offsets.

Before returning G-code, a separate input traversal checks every required
polyline segment, arc, dwell and plan waypoint against the ordered output, with
balanced operation/path boundaries. It checks command/motion/source consistency,
the process state at each source use, line mapping, operation ranges and program
completion. Repeated points, reversals and shared geometry each retain their
individual uses. Expected uses are streamed rather than copied into another full
motion list. `inspection.execution.completeness` records the successful counts.

Internal failures have code `INTERNAL_ERROR` and a structured `invariant`, command
or line context and source references where available. Preflight, translation
and the filter all perform the audit; failures publish no output. Existing
geometry/plan error codes and atomic file behavior are retained. No setting can
disable these checks or skip errors.

The audit describes offline source consumption and commanded state. It does not
read live offsets or verify physical movement. In particular, an unknown coolant
state after M6 remains unknown even if the requested value was off; this release
preserves the existing machine integration assumptions and output sequence.

## Existing state semantics

Before hashing or returning the final G-code string, the independent decoder in
`src/gcode-audit.js` matches its words and numbers against these command records.
`inspection.execution.serialization` has schema
`linuxcnc-next-nc/serialization-audit/1`, status, checked-line/executable/comment
counts and a scope statement. It complements `completeness`; neither status is
inferred from the other. Older archives show the missing audit as “Not recorded”.
Serialization failures retain code `INTERNAL_ERROR`, a `SERIALIZATION_*` invariant,
one-based line, observed text, expected command and source context. The same
verified string is hashed and returned. See [output verification](output-verification.md).

Translation builds an internal structured command for each output line. The
LinuxCNC formatter produces that line and its additive source-map fields together:

- `command`: semantic type and named parameters; motion uses XYZ program units
  and an explicit `work` or `machine` frame. Spindle CSS speed here is the emitted
  S value (metres/minute or feet/minute), while decoded input retains length/minute.
- `stateChange`: changed fields, with `before` and `after` values. `null` means
  unknown, not zero. No state is shared across translation calls.
- `modalState`: plane, feed, spindle, coolant, tool/H and WCS state at each motion
  or dwell. This describes emitted commands, not measured controller feedback.

G94/G95 explicitly reset the feed value before an F word establishes the new
value. M2 ends tracking; post-reset controller state is left unknown.

G53 waypoints record one machine-axis target. They do not create a known work
position or assume zero offsets. Work moves similarly do not invent machine
coordinates. M6 invalidates recorded modal/position knowledge until subsequent
commands reestablish it; no remap is executed by this tool. The requested-state
cache separately preserves existing emission/reset behavior. In particular,
unknown state after a custom M6 is not evidence that its spindle/coolant policy
was checked. Existing machine integration assumptions still apply.

Full circles preserve the commanded endpoint. Arc source starts/centers remain
separate from commanded starts and incremental center offsets. Polyline vertices,
including reversals and repeated points present in the input, remain ordered.

These are internal diagnostic records, not a new public input format or another
controller backend. `translate(text, plan, options)`, both profile versions,
execution-plan schemas 1–4, decoded fingerprints and `source-map/1` are retained.
Older archives render without these optional fields. The HTML report shows the
new command, state changes and state at each motion.

Eight pre-v0.6.0 golden fixture hashes guard exact G-code apart from the release
comment. Separate hand-authored lathe/mill programs exercise plane changes,
continue/link/retract boundaries, major arcs, reversals, feed changes and
independent T/H and WCS values. Native LinuxCNC tests compare complete ordered
canonical events against those programs, including nonzero offset-table values.
See [validation](validation.md) for evidence and limits.
