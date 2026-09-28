# v0.6.0 validation update

The shared-reader suite has 73 passing Node groups. Translator tests have 30 passing groups on Windows, with the POSIX filter group skipped there; two Python AXIS-helper tests pass. Eight golden fixture hashes retain exact pre-refactor G-code after the release comment and unchanged fingerprints. Both machines are checked in mm/inch.

New geometry tests cover radius/join tolerance boundaries, source records/lines and owning uses of shared geometry. CLI tests confirm identical preflight/translation failures, unchanged INVALID_NEXTNC codes, and no output on geometry or formatter failure. Unexpected exceptions are not relabeled as malformed geometry.

Command/state tests cover independent translations, unknown positions, M6 invalidation/reassertion, tool/H/WCS mappings, and repeated/reversed vertices in explicit and compacted paths. Source-association corruption is detected independently against decoded paths.

The expanded native suite compares every parsed canonical event (except comments) with independently hand-authored reference programs for lathe/mill in mm/inch. It adds nonzero, distinct T/H and G54/G55 values, changes of plane across continue/link/retract, major arcs, feed changes after rapid/dwell, spindle reversal and coolant changes. Five generated-program corruptions (plane, center, feed mode, spindle direction, H record) must either be rejected by rs274 or fail the semantic comparison. Existing full-circle and all-plane tests remain. Native CI results are pending at this commit.

The standalone canonical printer has four-decimal output. Comparisons allow 0.00011 in printed numeric fields, while Node tests retain exact-coordinate and threshold checks; canonical traces are not a sub-0.0001 precision oracle. CI archives synthetic inputs, reference/corrupted programs, tool/parameter tables and traces, including on failure. Nothing connects to a live machine.

# v0.5.0 validation update

The new suites cover XYZ mill paths in both units; all three principal arc planes, both senses and full circles; nonzero Y; two tools and independent H/WCS mappings; schema 4 and exact XYZ transitions; aggregated plan problems and skipped dependent checks; escaped HTML, old archives and exclusive report output; doctor/filter configuration precedence and failure isolation. POSIX CI also tests the real shell wrapper, paths with spaces/quotes and missing Node. The optional AXIS helper has process/argument tests; no AXIS GUI was launched.

A pre-change synthetic lathe snapshot retains its exact decoded fingerprint and all G-code after the release banner. The existing lathe suites remain in the test command. Standalone interpreter tests now include mill XY/XZ/YZ canonical arc endpoints, centers, directions, fixed-axis coordinates, linear XYZ moves and tool changes in mm/inch, starting from an inherited G7 state to verify G8 reset. Linux interpreter results are recorded by CI, not inferred from Windows tests.

The prior release's record follows for provenance; counts and package versions below describe those earlier observations.

# Validation scope

Version 0.4.1 development preview.

Header forgery and missing-reference CLI tests also verify empty G-code output and archived source record/line context. Six synthetic jobs and one private seven-operation job match 0.4.0 G-code exactly except for the release comment, with identical source maps. See [architecture research](architecture-research.md).

The 19 Node test groups include offline preflight, optional tool-table snapshots, independent T/H records, aggregated missing records, malformed/duplicate records, no-overwrite and no-partial-output behavior, explicit unverified scope, and exact motion/segment/dwell/transition source maps in both units. Native interpreter fixtures use the same synthetic tool-table snapshot that the translator checks. The filter's tool-table forwarding is also exercised in the native Linux test script.

The automated Node suites cover metric/inch output, CSS scale and cap, RPM/direction changes, G94/G95 feeds, arc senses/full circles, every linear vertex, dwell, coolant, logical tool/H/WCS mappings, separate repeated operations, machine transitions, exact decimal formatting and rejection before output. CLI tests preserve inputs/existing outputs, check stale/incomplete plans, archive diagnostics, retain the last error after success, and block active G-code/comment injection from names.

Continuation tests cover exact matching boundaries, changed feeds, retained legacy/explicit retracts, first-operation rejection, incompatible tool/H/WCS, even a 1e-12 entry difference, spindle speed/mode/direction/cap changes, coolant changes, and final arc/dwell state. A separate modal decoder reconstructs every emitted body vertex to check lossless omission of unchanged axis words. Native tests also interpret two consecutive operations in metric/inch, verifying actual cutting endpoints, both feeds and one spindle start.

Link tests cover ordered multi-waypoint and single-axis connections, inherited unchanged coordinates, missing/oversized/invalid/conflicting paths, exact endpoint checks, first-operation rejection, incompatible process state, source-map waypoint context and legacy schema 1/2 behavior. Synthetic native metric/inch link cases verify the actual canonical rapid waypoints in order, cutting endpoints/feeds and one spindle start. Compatibility checks do not test collision clearance or physical machine travel.

GitHub CI runs these suites on Linux and Windows with Node 20, 22 and 24. It also runs generated metric and inch programs through LinuxCNC's offline `rs274` interpreter in a Debian bookworm container using synthetic tool and parameter files. The interpreter test checks normal program completion, actual canonical arc endpoints/centres/senses, CSS speed and cap, RPM and feed mode/rate. Package versions and output are evidence for that bounded interpreter test only.

Observed interpreter package: Debian `linuxcnc-uspace 2.9.0~pre1+git20230208.f1270d6ed7-1+deb12u2`. The canonical G18 trace confirms Z/X endpoint ordering, positive rotation for the CCW quarter arc and negative rotation for both the CW return and full circle. The metric CSS sample becomes S80; the inch sample becomes S100; both retain the 1800 RPM cap and G95 F0.1. This package is not asserted to match the user's installed LinuxCNC build.

Private Fusion exports can be inspected and translated locally with simulation-only plans. They are not uploaded as public fixtures. A generated G-code file is not proof of toolpath agreement with unavailable CAD/stock/fixtures or of machine-safe transitions.

Not verified: full EXPRESS/AP238 conformance, arbitrary third-party STEP-NC inputs, LinuxCNC GUI filter loading on the user's computer, real M6 remaps, spindle feedback/at-speed wiring, actual tool/WCS offsets, path clearance, stop/resume/abort/restart or physical machine execution. The current Windows development host has no installed LinuxCNC interpreter; native interpreter validation runs in isolated Linux CI.
