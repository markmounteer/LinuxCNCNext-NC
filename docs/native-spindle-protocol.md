# Stage 4 native spindle protocol

Status: Rust preparation/protocol checkpoint; **not native execution qualification**.
The controller's measured-feedback planner checkpoint is separate. The matching
task host, runtime fault matrix and machine acceptance remain required. No
controller was contacted, enabled or modified by these changes.

## Explicit capability binding

An ABI-2 snapshot carries `nextnc_spindle_evidence`. All-zero evidence disables
spindle synchronization/CSS. There are no default feedback allowances. Bit 1
means spindle-zero velocity feed; bit 2 adds CSS and requires bit 1. Unknown flags,
nonzero reserved fields, missing identity, incomplete/nonfinite/nonpositive
policy values, and relative error outside [0,1) are rejected. A capability on an
XYZ mill or with engaged shaping is rejected even for a G94-only job. Reverse
spindle direction still requires the existing explicit reverse capability.

The trusted host must establish the identity from the compatible native task,
motmod, shim, motion-feedback producer, connected channels and execution order.
It must read back the active feedback policy. An INI flag or a caller-supplied
digest does not prove any of this. Rust validates the evidence's shape and binds
it, but cannot independently authenticate HAL wiring or an installed binary.
The host must continue checking live compatibility/fault state while it owns
execution. Threading and rigid tapping are not granted by velocity feed.

The fingerprint includes the identity, capability flags and all five policy
values: maximum measured RPS, heartbeat timeout, comparison window, absolute
position error in revolutions, and relative error. A change invalidates a bound
start or held resume. Tool-procedure adoption retains the existing worker
rebind, fresh-fingerprint, physical-result and owner-generation checks.

## Feed law and message accounting

Line/circle/stationary messages with `NEXTNC_FEED_PER_REV` carry positive
`feed_mm_rev`, with `feed_mm_s` zero. Other cutting messages carry `feed_mm_s`.
X coordinates and F values are physical radius millimetres; G7 display and
program inch units cannot double or rescale the stored feed a second time.
For synchronized motion the scalar velocity is the geometry/axis ceiling,
not F multiplied by a requested or nominal RPM. The realtime planner evaluates
the actual demand from fresh measured spindle motion and feed override. Spindle
override must not be applied again to that measured rate.

`NEXTNC_SPINDLE_SYNC` (kind 5) is an independently receipted piece emitted only
when the hardware synchronization rate changes. Its `feed_mm_rev` is positive to
select spindle-zero velocity synchronization, or zero to clear it for G94/G0.
The host scales that distance by external length units and supplies spindle 0
and velocity mode 1 to `EMC_TRAJ_SET_SPINDLESYNC`. The separate source motion
selects the interpreter's modal feed. In particular, clearing hardware sync
for a rapid must preserve the previous interpreter F and feed mode.

Sync, termination and geometry pieces retain the source command identity and
ordered piece indexes. There is no extra per-vertex barrier; the motion shim
already retains each queued segment's feed law. `RestoreFeedPerMinute` explicitly
clears synchronization and the modal F value at the end or reset boundary.

Motion-recovery receipts carry `feed_known`, `feed_per_rev` and separate
`feed_mm_s`/`feed_mm_rev` values. They describe the accepted piece prefix, never
physical execution. A rapid inherits the previous modal feed and its units,
including when it is the last accepted motion at abort. Hardware sync pieces
cannot rewrite that inherited modal state. Actual commanded-pose receipts,
drain, reconciliation and the existing motion-owner lease remain mandatory.

## CSS demand and offsets

`NEXTNC_SPINDLE` with `NEXTNC_CSS` carries:

| Field | Meaning |
| --- | --- |
| `argument` | +1 clockwise or -1 counterclockwise |
| `value` | Positive surface speed, mm/s |
| `css_factor_rpm_mm` | Surface speed times 60 / (2 pi) |
| `css_maximum_rpm` | Positive source cap, at most machine policy maximum |
| `css_x_offset_mm` | Combined current WCS, G92 and tool X, in radius mm |

The thin host scales the factor and X offset to external machine length units,
applies direction to the spindle speed/factor, and dispatches the existing
spindle-on message. Existing motmod owns radius-dependent RPM, overrides and
the cap, including the zero-radius case. Requested CSS speed never becomes the
motion-feedback channel.

If an offset changes while CSS is active, Rust emits `NEXTNC_CSS_UPDATE` (kind
22) after that offset's state piece. It carries the retained CSS rate/cap/direction
and newly combined X offset. The host uses a spindle-speed update, not another
spindle start. Tool-procedure rebinding resolves subsequent CSS against the
new actual tool table. RPM/reset events clear the active CSS tracking.

## Verification and remaining work

The four pinned-producer fixtures cover mm/inch, forward/reverse spindle, G95
lines and analytic circles, rapid/G94/G95 transitions, changed F, RPM/CSS changes,
two work offsets and two physical tools with independent H offsets. Tests also
cover missing/invalid capability evidence, reverse/shaping/mill refusal,
fingerprint drift, changed tool geometry, G95 with constant RPM, abort during a
rapid, CSS offset-update ordering and arithmetic overflow. C11 and C++17 layout
checks match the Rust ABI extents and field offsets.

The simulator must still consume these messages through the pinned task host,
custom motmod and motion shim. That gate includes measured-versus-commanded/PID
channel independence, spindle ramps/stops/reversal, missing/stale/nonfinite
feedback, feed/spindle overrides, CSS radius/offset/cap, mixed queues, and actual
fault-stop/recovery behavior. The previous RS274 G95 checkpoint does not prove
native G95/CSS execution. Stage 4 remains open until the independent motion oracle
and rejection/fault matrix pass through the complete native stack.
