# Validation scope

Version 0.1.0 development preview.

The automated Node suites cover metric/inch output, CSS scale and cap, RPM/direction changes, G94/G95 feeds, arc senses/full circles, every linear vertex, dwell, coolant, logical tool/H/WCS mappings, separate repeated operations, machine transitions, exact decimal formatting and rejection before output. CLI tests preserve inputs/existing outputs, check stale/incomplete plans, archive diagnostics, retain the last error after success, and block active G-code/comment injection from names.

GitHub CI runs these suites on Linux and Windows with Node 20, 22 and 24. It also runs generated metric and inch programs through LinuxCNC's offline `rs274` interpreter in a Debian bookworm container using synthetic tool and parameter files. The interpreter test checks normal program completion and all expected arc moves. Package versions and output are evidence for that bounded interpreter test only.

Private Fusion exports can be inspected and translated locally with simulation-only plans. They are not uploaded as public fixtures. A generated G-code file is not proof of toolpath agreement with unavailable CAD/stock/fixtures or of machine-safe transitions.

Not verified: full EXPRESS/AP238 conformance, arbitrary third-party STEP-NC inputs, LinuxCNC GUI filter loading on the user's computer, real M6 remaps, spindle feedback/at-speed wiring, actual tool/WCS offsets, path clearance, stop/resume/abort/restart or physical machine execution. The current Windows development host has no installed LinuxCNC interpreter; native interpreter validation runs in isolated Linux CI.
