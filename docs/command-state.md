# Command and state records (v0.6.0)

Translation builds an internal structured command for each output line. The
LinuxCNC formatter produces that line and its additive source-map fields together:

- `command`: semantic type and named parameters; motion uses XYZ program units
  and an explicit `work` or `machine` frame. Spindle CSS speed here is the emitted
  S value (metres/minute or feet/minute), while decoded input retains length/minute.
- `stateChange`: changed fields, with `before` and `after` values. `null` means
  unknown, not zero. No state is shared across translation calls.
- `modalState`: plane, feed, spindle, coolant, tool/H and WCS state at each motion
  or dwell. This describes emitted commands, not measured controller feedback.

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
