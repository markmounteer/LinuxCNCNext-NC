# Validation scope

Version 0.3.0 development preview.

The automated Node suites cover metric/inch output, CSS scale and cap, RPM/direction changes, G94/G95 feeds, arc senses/full circles, every linear vertex, dwell, coolant, logical tool/H/WCS mappings, separate repeated operations, machine transitions, exact decimal formatting and rejection before output. CLI tests preserve inputs/existing outputs, check stale/incomplete plans, archive diagnostics, retain the last error after success, and block active G-code/comment injection from names.

Continuation tests cover exact matching boundaries, changed feeds, retained legacy/explicit retracts, first-operation rejection, incompatible tool/H/WCS, even a 1e-12 entry difference, spindle speed/mode/direction/cap changes, coolant changes, and final arc/dwell state. A separate modal decoder reconstructs every emitted body vertex to check lossless omission of unchanged axis words. Native tests also interpret two consecutive operations in metric/inch, verifying actual cutting endpoints, both feeds and one spindle start.

Link tests cover ordered multi-waypoint and single-axis connections, inherited unchanged coordinates, missing/oversized/invalid/conflicting paths, exact endpoint checks, first-operation rejection, incompatible process state, source-map waypoint context and legacy schema 1/2 behavior. Synthetic native metric/inch link cases verify the actual canonical rapid waypoints in order, cutting endpoints/feeds and one spindle start. Compatibility checks do not test collision clearance or physical machine travel.

GitHub CI runs these suites on Linux and Windows with Node 20, 22 and 24. It also runs generated metric and inch programs through LinuxCNC's offline `rs274` interpreter in a Debian bookworm container using synthetic tool and parameter files. The interpreter test checks normal program completion, actual canonical arc endpoints/centres/senses, CSS speed and cap, RPM and feed mode/rate. Package versions and output are evidence for that bounded interpreter test only.

Observed interpreter package: Debian `linuxcnc-uspace 2.9.0~pre1+git20230208.f1270d6ed7-1+deb12u2`. The canonical G18 trace confirms Z/X endpoint ordering, positive rotation for the CCW quarter arc and negative rotation for both the CW return and full circle. The metric CSS sample becomes S80; the inch sample becomes S100; both retain the 1800 RPM cap and G95 F0.1. This package is not asserted to match the user's installed LinuxCNC build.

Private Fusion exports can be inspected and translated locally with simulation-only plans. They are not uploaded as public fixtures. A generated G-code file is not proof of toolpath agreement with unavailable CAD/stock/fixtures or of machine-safe transitions.

Not verified: full EXPRESS/AP238 conformance, arbitrary third-party STEP-NC inputs, LinuxCNC GUI filter loading on the user's computer, real M6 remaps, spindle feedback/at-speed wiring, actual tool/WCS offsets, path clearance, stop/resume/abort/restart or physical machine execution. The current Windows development host has no installed LinuxCNC interpreter; native interpreter validation runs in isolated Linux CI.
