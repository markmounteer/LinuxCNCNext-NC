# v0.12.0 validation update

Local Windows / Node 24.13.1: **104 Node groups passed, one POSIX-only filter
group skipped**, plus both Python AXIS-helper tests. The isolated independent
parser compiled its 31 pinned files and passed all 56 cases with zero unexpected
outcomes. Existing golden checks retain fingerprints and exact G-code after
the version comment; regenerating the checked-in examples changes only that
comment from v0.11.0.

Eleven new Node groups cover small and major arcs across angular wrap, both
full-circle directions, the lathe XZ and mill XY/XZ/YZ planes, mm/inch scaling,
zero-length vertices, partial-axis distances, unknown positions, separate work
offsets, G94/G95 RPM time and unknown CSS time. Stock tests independently count
plunge cells and compare a slot/circle union, retain rapid-contact warnings
without removing stock, and reject missing geometry, mismatched bindings,
unsupported shapes/frames and exceeded sample/cell budgets. CLI tests preserve
the archive, stock setup and G-code, reject preview flags on translation, and
leave no output on failed previews. CSP tests bind one trusted script and keep
hostile archive content as escaped data.

Browser verification used the locally served synthetic HTML in the Codex
in-app browser. Lathe XZ and milling toolpaths, full-circle selection, clicking
a segment, coordinate frames, operation/cut filters, zoom/fit, source/state
details and stock exclusion-line navigation were checked. The stock surface
was inspected visually after fixing offscreen-canvas redraw. Direct file://
navigation could not be tested because the browser tool blocks that protocol;
the HTML has no network dependencies and its CSP hash normalizes CRLF.

The stock model is approximate and mill-only; it excludes unknown/G53 moves,
holders, fixtures and unsupported cutter shapes. No FreeCAD binary, physical
controller, machine motion, live tool table or machine acceptance was tested.
See [the review contract and limits](visual-review.md).

# v0.11.0 validation update

Local Windows / Node 24.13.1: **93 Node groups passed, one POSIX-only filter
group skipped**, and both Python AXIS-helper tests passed. The independent
parser harness again passed all **56 cases (20 valid, 36 invalid)** with no
unexpected outcomes; all 31 pinned upstream source objects were verified.

The original eight golden hashes and fingerprints are retained. Tests remove
only the deliberate three reset lines after each M6 initialization before
comparison with those old hashes. Example regeneration changes only the version
banners and those same reset lines; source examples and reviewed plans are
unchanged. Installed package inspection excludes the development compiler,
parser harness and tests; no production dependencies were added.

New policy tests reject all 16 omissions from the Mastercam review and cover
startup, first/later M6, every restoration command, missing initial operation
state, late preparation, premature offset cancellation, early restart,
wrong-boundary stops and misleading emitter state. They also verify that known
state can satisfy obligations without redundant commands. CLI/preflight/filter
failures leave stdout empty, preserve existing destinations, retain expected
and observed state plus boundary/plan context, and mark serialization unchecked.
Old JSON/HTML archives retain missing audit data as "Not recorded".

Twelve new synthetic state scenarios (both machines/units x off/flood/mist)
match hand-authored instruction sequences: equal-RPM reversal, equal-value
G94/G95 changes, rapid-to-feed, dwell-only changes, lathe CSS-cap-only changes,
equal emitted S across RPM/CSS modes, independent T/H/WCS, and same-tool retracts.
Targeted negative geometry tests retain rejection of negative/tilted normals,
helices, changed coordinate orientation and cutter-contact compensation.

All **nine jobs passed** in [CI run 36501739274](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36501739274)
at implementation commit `b7847bf61f2c90a2f680e2f1a92268b467df11f5`: six
Windows/Linux Node 20/22/24 jobs, two independent-parser jobs and standalone
LinuxCNC. Linux passed all 94 Node groups; Windows passed 93 with one POSIX-only
skip. Both Python helper tests and example reproducibility passed in each Node
job. Both parser jobs passed all 56 cases and retained identical compiled-parser
SHA-256 `e241a05a73726a6d97f2c87f00f07d3c24e2b920036ccecbf3fefe644d4db06f`.

Downloaded native artifacts contain **96 programs and 96 traces: 32 valid
scenarios, 24 independent references and 40 deliberate corruptions**. This
adds 12 process-state scenarios and 12 corresponding hand-authored references.
Complete ordered canonical comparisons passed. Interpreter package:
`linuxcnc-uspace 2.9.0~pre1+git20230208.f1270d6ed7-1+deb12u2`.
The four-decimal canonical printer and 0.00011 comparison allowance are
unchanged; separate Node checks retain exact output-number comparisons.

These are offline results. Installed M6 remaps, feedback, actual offsets,
clearance and physical execution remain untested; general EXPRESS/AP238
conformance remains unchecked.

# v0.10.0 validation update

Local Windows / Node 24.13.1: **80 Node groups passed, one POSIX-only filter
group skipped**, plus both Python AXIS-helper tests. New regressions reject all
12 previously accepted numeric metadata substitutions (three for each
machine/unit pair), distinguish inherited fields, required/optional/derived
values, typed measures, references, aggregates and complex components, and
retain Unicode metadata. CLI tests verify shape failures produce no output,
preserve existing destinations, and archive the exact field and stage coverage.
Fault-injection tests verify failed command and text audits leave subsequent
stages unchecked. General EXPRESS/full AP238 stages are never inferred passed.

The isolated independent parser check passed **56 cases: 20 valid and 36
invalid controls**, with no unexpected outcomes. Source bytes were verified
against all 31 pinned upstream Git objects; source and ordinary TypeScript
build hashes are enforced. Cases preserve all normalized values/references and
record the upstream parser's known omissions as named stage discrepancies.
The harness does not invoke the experimental schema loader. See its
[scope and reproducible command](../tools/step-conformance/README.md).

Eight golden programs retain exact G-code after the banner and original
fingerprints. Example regeneration changes only the two version banners. An
`npm pack --dry-run` inspection confirms development tooling and compiler
dependencies are excluded from the installed translator. The production
writer/shared reader, profiles, execution-plan schemas and machine settings
are unchanged.

All **nine jobs passed** in [CI run 36493004100](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36493004100)
at code commit `27521300850d088893ddfcf479b09dd249a4bd2a`: six Windows/Linux
Node 20/22/24 jobs, two independent-parser jobs and standalone LinuxCNC.
Linux passed 81 Node groups; Windows passed 80 with the POSIX-only skip.
Both Python helper tests and example reproducibility passed in each Node job.
Both parser jobs passed all 56 cases and produced identical compiled-parser
SHA-256 `e241a05a73726a6d97f2c87f00f07d3c24e2b920036ccecbf3fefe644d4db06f`.
The downloaded artifacts retain all case outcomes, hashes, spans and inputs.

The native suite passed 20 valid scenarios, 12 independent reference programs
and 40 deliberate corruptions (72 interpreted programs, confirmed in the
downloaded artifact). Interpreter package:
`linuxcnc-uspace 2.9.0~pre1+git20230208.f1270d6ed7-1+deb12u2`.
The canonical printer's four-decimal precision and 0.00011 comparison allowance
remain unchanged. These are offline results, not physical machine acceptance
or full EXPRESS/AP238 conformance.

# v0.9.0 validation update

Local Windows / Node 24.13.1: **72 Node groups passed, one POSIX-only filter group
skipped**, and both Python AXIS-helper tests passed. Example regeneration changes
only the two version banners. Eight golden programs retain exact G-code after
the banner and their original fingerprints; the existing 32-case geometry/unit
matrix now also passes through the automatic final-text audit.

New serialization tests reject 144 text-only corruptions with unchanged command
records: 33 cases for each machine/unit pair, two additional CSS cases for each
lathe unit system, and two full-circle cases for each pair. The 20 faults from the
sixth-repository review are included. They cover arc sense, fractional feeds,
G53 frames, spindle direction, M2, axes/centres, initialization modes, T/H/WCS,
coolant, dwell, malformed numbers, duplicate/extra words and comment injection.
Additional checks cover trailing/missing text and passive prefixes even when
a command and its text agree on an active directive. CLI tests prove empty
stdout, no partial publication, preserved existing destinations and archived
line/operation/STEP context.

Saved-file identity tests cover both machines/units and real translation and
preflight archives, exact-byte changes (coordinates, line endings, BOMs and
comments), missing/invalid hashes without fallback, archive-only compatibility,
escaped metadata, exclusive HTML output and unchanged diagnostic indexes/inputs.
Resource tests enforce the 64 MiB limit on actual reads, reject missing/non-regular
files, detect a changing file, and test nonblocking FIFO rejection on POSIX.

All seven jobs passed in [CI run 36476468764](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36476468764)
at `39337423e8bd57825f5cf4017902857b09179d1e`: Windows/Linux Node 20/22/24 and
the standalone LinuxCNC interpreter. Linux passed all 73 Node groups; Windows
passed 72 with the POSIX-only skip. Both Python helper tests and example
reproducibility passed in every Node job.

The existing native suite passed 20 valid scenarios, 12 independent reference
programs and 40 deliberate corruptions (72 interpreted programs). Downloaded
evidence contains 72 `.ngc` programs and 72 trace files. This includes lathe XZ
and mill XY/XZ/YZ in both units, major/full arcs, nonzero independent T/H/WCS,
process changes and ordered transitions. Interpreter package:
`linuxcnc-uspace 2.9.0~pre1+git20230208.f1270d6ed7-1+deb12u2`.
The canonical printer's four-decimal precision and 0.00011 comparison allowance
remain unchanged; exact-number Node tests provide the separate precision checks.

These are offline tests, not physical machine acceptance. Profiles, CPS/shared
reader, execution-plan schemas and machine configuration are unchanged.

# v0.8.0 validation update

Local Windows validation: 62 Node groups passed, one POSIX-only filter group
skipped, and both Python AXIS-helper tests passed. Eight golden programs retain
their exact G-code after the release banner and their existing fingerprints.
The CPS, shared reader, profiles and execution-plan schemas are unchanged.

New report tests cover both machines and both units, later spindle/feed/coolant
changes, CSS conversion, G94/G95, dwells, inherited and unknown M6 state,
source/command bindings, shutdown exclusion, tool-table evidence, escaped HTML
and old archives. All STEP-origin process commands are represented in the summary.

The geometry matrix covers lathe XZ and mill XY/XZ/YZ, all four sign quadrants,
and physically equivalent mm/inch jobs (32 cases, plus corresponding unshifted
baselines). It compares G-code decoded independently of the source map against
explicit reference programs and translated-coordinate expectations. Cases
include 270-degree arcs across the angular wrap, both full-circle senses,
repeated/reversed vertices, reviewed links, non-shifted G53 moves and exact
commanded positions across reader-tolerated joins. Deliberately wrong unit scale,
arc sense, plane, centre sign and G53/work-frame treatment must be detected.

All seven jobs passed in [CI run 36468682474](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36468682474)
at `a827d3578a32427430b90893de2016e33e42b068`: Windows/Linux Node 20/22/24 and
the standalone LinuxCNC interpreter. Linux passed all 63 Node groups; Windows
passed 62 with the POSIX-only skip. Both Python helper tests and example
reproducibility passed across the six Node jobs.

Native checks passed 20 valid scenarios, 12 independent reference programs and
40 deliberate corruptions (72 interpreted programs). Eight scenarios, eight
references and 20 corruptions are new in this release. They use nonzero T/H and
WCS values in the existing explicit inch-site fixture convention. The first CI
run caught an H1/H2 mismatch in the new synthetic plan/reference; the fixture
now explicitly maps H2 and a Node assertion guards that mapping. No production
motion change was needed. The interpreter package was
`linuxcnc-uspace 2.9.0~pre1+git20230208.f1270d6ed7-1+deb12u2`.
The documented four-decimal trace tolerance does not relax exact-number Node
tests or production output. Synthetic traces were downloaded and their counts
reconciled locally. Lathe and mill HTML reports were also browser-reviewed;
five-column table widths and process labels were corrected after that review.

These tests do not connect to hardware or establish physical machine acceptance.
Earlier release evidence follows.

# v0.7.0 validation update

Local Windows validation: 35 translator Node groups passed, one POSIX-only filter group skipped, and two Python AXIS-helper tests passed. The shared Fusion reader has 74 passing Node groups and its generated-post build check passed. Examples were regenerated; only the translator release banners changed. Eight golden programs retain their fingerprints and exact G-code after that banner. Signed-zero inputs retain the existing numeric semantics without a new tolerance.

New tests cover both machines and units: successful STEP record/line/column provenance, shared geometry and derived full-circle sources, raw input hashes, renumbered/reformatted inputs, plan JSON Pointers, atomic rejection of invalid internal commands, and ordered output auditing. Fault injection drops or duplicates motions/dwells/waypoints, swaps link waypoints, and corrupts source ownership, arc centers, motion details, line numbers, action types or process state. Each must fail with its intended internal invariant. CLI preflight, translation and filter failures retain structured local diagnostics and publish no output.

All seven jobs passed in [translator CI run 36460178068](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36460178068) at code commit `1c0c8a11ee8761dab2484086bb10012fb55d0f05`: Windows/Linux with Node 20/22/24 plus the standalone LinuxCNC interpreter. Linux ran all 36 Node groups; Windows passed 35 with the one POSIX-only skip. Both Python helper tests and example reproducibility passed on all six Node jobs.

Native verification passed all 12 machine/unit scenarios, including the independent reference comparisons and their 20 intentional corruptions. Four additional hand-authored reference programs were interpreted. The interpreter package was `linuxcnc-uspace 2.9.0~pre1+git20230208.f1270d6ed7-1+deb12u2`; precision and fixture-unit limits remain as documented below. [Shared-reader CI run 36459657876](https://github.com/markmounteer/Fusion360Next-NC/actions/runs/36459657876) passed all four jobs at `a6e64622343c41545925ebf6e8dc91e12d320ac1`, including Node 20/22/24, generated-post checks and Windows diagnostic collector tests.

These tests do not connect to hardware or establish physical machine acceptance. The following v0.6.0 section retains the preceding release's evidence for provenance.

# v0.6.0 validation update

The shared-reader suite has 73 passing Node groups. Translator tests have 30 passing groups on Windows, with the POSIX filter group skipped there; two Python AXIS-helper tests pass. Eight golden fixture hashes retain exact pre-refactor G-code after the release comment and unchanged fingerprints. Both machines are checked in mm/inch.

New geometry tests cover radius/join tolerance boundaries, source records/lines and owning uses of shared geometry. CLI tests confirm identical preflight/translation failures, unchanged INVALID_NEXTNC codes, and no output on geometry or formatter failure. Unexpected exceptions are not relabeled as malformed geometry.

Command/state tests cover independent translations, unknown positions, M6 invalidation/reassertion, tool/H/WCS mappings, and repeated/reversed vertices in explicit and compacted paths. Source-association corruption is detected independently against decoded paths.

The expanded native suite compares every parsed canonical event (except comments) with independently hand-authored reference programs for lathe/mill in mm/inch. It adds nonzero, distinct T/H and G54/G55 values, changes of plane across continue/link/retract, major arcs, feed changes after rapid/dwell, spindle reversal and coolant changes. Five generated-program corruptions (plane, center, feed mode, spindle direction, H record) must either be rejected by rs274 or fail the semantic comparison. Existing full-circle and all-plane tests remain. Native CI passed: [translator run 36452446035](https://github.com/markmounteer/LinuxCNCNext-NC/actions/runs/36452446035) at code commit `2896c25`. All seven jobs passed (six Node OS/version combinations plus LinuxCNC). All 12 valid machine/unit scenarios and 20 deliberately corrupted programs passed their expected checks; four additional hand-authored reference programs were interpreted. The [shared-reader CI](https://github.com/markmounteer/Fusion360Next-NC/actions/runs/36451633768) also passed at `cfda077`, including its Windows collector tests.

The standalone canonical printer has four-decimal output. Comparisons allow 0.00011 in printed numeric fields, while Node tests retain exact-coordinate and threshold checks; canonical traces are not a sub-0.0001 precision oracle. CI archives synthetic inputs, reference/corrupted programs, tool/parameter tables and traces, including on failure. Nothing connects to a live machine.

The new offset fixtures specify an **inch machine/table** and test both G20 and
G21 programs, asserting the interpreted conversion of nonzero offsets. Debian's
2023 standalone build fixes external units to inches even with a millimetre
TRAJ setting; the newer 2.9 driver supports that setting. The fixture uses inch
TRAJ units explicitly for portability rather than silently assuming its table
follows the program units. This does not change the translator or the user's
machine configuration. Sources: [packaged driver](https://github.com/LinuxCNC/linuxcnc/blob/f1270d6ed7/src/emc/sai/driver.cc#L677)
and archived interpreter traces.

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
