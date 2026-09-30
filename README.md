# LinuxCNCNext-NC

[![Tests](https://github.com/markmounteer/LinuxCNCNext-NC/actions/workflows/ci.yml/badge.svg)](https://github.com/markmounteer/LinuxCNCNext-NC/actions/workflows/ci.yml)

An experimental translator and LinuxCNC input filter for XZ lathe and fixed-axis XYZ milling files from [Fusion360Next-NC](https://github.com/markmounteer/Fusion360Next-NC).

It reads the `next-nc/turning-toolpath/0.1` and `next-nc/milling-toolpath/0.1` profiles, validates it, and produces LinuxCNC RS274 G-code. LinuxCNC supplies preview, interpretation, trajectory planning, limits and HAL. This is a translation bridge, not a native STEP-NC interpreter or a general AP238 implementation. No LinuxCNC machine configuration or hardware is changed by installing this repository.

## Quick start

Version 0.12.0 adds an offline interactive toolpath reviewer for lathe and XYZ
mill, motion-distance and ideal-feed-time summaries, and an optional approximate
XYZ stock-removal preview. Select a G-code line to see its geometry, retained
state and original STEP/plan source. Work offsets and machine coordinates stay
separate; unknown moves are listed instead of drawn from an invented origin.
Fusion remains the CAD/CAM source. Existing machining output is unchanged from
0.11.0 except for the version comment. No new machining options or runtime
dependencies are introduced. See [visual review and stock preview](docs/visual-review.md).
See [policy verification](docs/policy-verification.md),
[profile validation](docs/profile-validation.md), [output verification](docs/output-verification.md),
[process summaries](docs/process-summary.md),
[command/state records](docs/command-state.md) and [validation](docs/validation.md).

Requires Node.js 20 or later; the translator has no npm dependencies. The optional independent parser development tool has its own compiler lockfile and is excluded from the installed package. LinuxCNC is needed only for preview/execution and the optional standalone interpreter tests.

```sh
git clone https://github.com/markmounteer/LinuxCNCNext-NC.git
cd LinuxCNCNext-NC
npm test
node bin/nextnc.js inspect examples/synthetic.stpnc
node bin/nextnc.js translate examples/synthetic.stpnc --plan examples/simulation-plan.json --output /tmp/nextnc-simulation.ngc
```

The example coordinates and tool table assumptions are **for simulation only**. They are not settings for a Grizzly or any physical lathe or mill. See `examples/synthetic-mill.stpnc` and `examples/simulation-mill-plan.json` for the XYZ counterpart.

## Translate your own export

```sh
node bin/nextnc.js inspect /path/1001.stpnc
node bin/nextnc.js plan-template /path/1001.stpnc --output /path/1001-plan.json
# Fill and review the template's mappings and transition paths.
node bin/nextnc.js preflight /path/1001.stpnc --plan /path/1001-plan.json --tool-table /path/to/config/tool.tbl
node bin/nextnc.js translate /path/1001.stpnc --plan /path/1001-plan.json --tool-table /path/to/config/tool.tbl --output /path/1001.ngc
```

The template deliberately has `null` mappings and required approach/retract paths. The export contains each operation's entry point, but no machine-safe approach or tool-change policy. Translation requires explicit LinuxCNC tool/H-offset and work-offset mappings, plus ordered machine-coordinate retract and work-coordinate approach waypoints where needed. It never assumes work offset 0 means the currently active WCS or that a straight rapid between operations is safe.

Lathe templates use execution-plan schema 3; XYZ milling templates use schema 4 with `machine: "mill"`. The program profile determines the machine type; it is not a machining option. When adjacent operations have exactly matching exit/entry, Fusion tool/offset/WCS, spindle state and coolant, the template marks the boundary `{"mode":"continue"}`. Validation rechecks these conditions and mapped offsets. Such a boundary keeps the spindle running and adds no retract or approach.

When compatible operations have different exit/entry points, schema 3 can accept an explicitly reviewed `{"mode":"link","moves":[...]}` path in work coordinates while preserving process state. The template **never invents this path**; it leaves a retract boundary until you supply a reviewed connection. Waypoints are one axis at a time and must reach the exact next entry. Existing schema 1/2 plans and explicit retract boundaries keep their reviewed paths. Inspect reports explain eligibility. See [execution plans](docs/execution-plan.md).

An execution plan is bound to the exact decoded program fingerprint. A change to tooling, units, offsets, paths, feeds or process states requires a reviewed plan for the new program. Export timestamps, record numbering and equivalent geometry sharing do not invalidate it. See [execution plans](docs/execution-plan.md).

The entire input and plan are checked before any G-code reaches stdout or an output file. Existing files are never overwritten. The final output path is created atomically; failed writes do not leave a partial program there.

## Offline preflight

`preflight` performs the same translation checks but returns a JSON review instead of G-code. `--output` saves that JSON to a new file. Optional `--tool-table` checks a snapshot of your existing LinuxCNC table for syntax, duplicate tool numbers and the presence of every mapped T and H record. Missing records are reported together with all affected operations. Supply the same table to `translate` to repeat the check at generation time; preflight is not a reusable authorization token.

The default filename is `tool.tbl`; use the file specified by your existing `[EMCIO]TOOL_TABLE` setting. No table is written or loaded into LinuxCNC. Without `--tool-table`, the report explicitly says `not_checked`. Presence of a record does not establish correct physical tooling, calibrated offsets, changer pockets, work offsets, clearance or safe machine operation; an external tool database or live controller may differ from the file snapshot. See [preflight and source maps](docs/preflight.md).

## Review and installation tools

```sh
node bin/nextnc.js doctor
node bin/nextnc.js report /path/to/timestamped-diagnostic.json --output /path/new-review.html
# Optionally check a saved file against the report's candidate hash:
node bin/nextnc.js report /path/to/timestamped-diagnostic.json --gcode /path/job.ngc --output /path/new-checked-review.html
# Optional fixed-XYZ stock preview, with explicitly supplied stock/tool geometry:
node bin/nextnc.js report /path/to/timestamped-diagnostic.json --stock-setup /path/stock.json --output /path/new-stock-review.html
# Generate standalone synthetic mill/lathe HTML demonstrations:
npm run example:review
```

`doctor` and the LinuxCNC filter use the same resolver for `NEXTNC_PLAN`, `NEXTNC_TOOL_TABLE` and `NEXTNC_DIAGNOSTICS`. Doctor reports runtime, resolved paths, file readability and diagnostics-directory access without creating files. Run preflight with your program for job validation.

`report --gcode` compares exact file bytes with the archived candidate G-code hash, including for preflight archives. A match returns exit 0; a mismatch or missing/invalid candidate hash produces a clearly marked HTML report and exit 1. Without `--gcode`, the report says “Not checked”. Line endings and BOMs are not normalized. Selected files must be regular files of at most 64 MiB. No file is loaded into LinuxCNC or executed. See [output verification](docs/output-verification.md).

The HTML report shows job hashes, grouped tools, mappings, process state, transition decisions and operation links to source lines. It is self-contained, escapes file content, writes only to a new destination and leaves existing diagnostic archives unchanged. Independent execution-plan problems are reported together with operation names and JSON field locations. Checks needing invalid mappings are explicitly marked not completed; malformed files and mismatched fingerprints stop immediately. [Details](docs/preflight.md). An [optional AXIS shortcut](docs/axis-report.md) opens an explicitly selected archive without replacing AXIS or assuming it matches the loaded job.

## Supported translation

| Next-NC content | LinuxCNC output |
| --- | --- |
| Millimetres / inches, X radius, XZ plane | G21 / G20, G8, G18 |
| Absolute positions, relative arc centres | G90, G91.1 |
| Rapid/cutting polylines | Every vertex in order as G0/G1; unchanged axis words omitted without rounding |
| XYZ milling, fixed +Z tool axis | G17 initially; G17/G18/G19 with I/J, I/K or J/K for principal-plane arcs |
| CW/CCW arcs and full circles | G2/G3; full circles omit endpoint axes |
| Feed per minute / revolution | G94 / G95 with explicit F on every feed-mode change |
| Constant RPM | G97 S, M3/M4 on spindle 0 |
| Lathe CSS and maximum RPM | G96 S D; mm/min converted to m/min, inch/min to ft/min |
| Off / flood / mist coolant | M9 / M8 / M7 |
| Dwell | G4 P in seconds |
| Reviewed logical tool/offset mapping | Tn M6, G43 Hn; no repeated M6 for the same mapped tool |
| Reviewed work-offset mapping | G54 through G59.3 |

G8 establishes ordinary X coordinates (radius on a lathe) even after a prior G7 program. G40 prevents a second application of cutter/nose-radius compensation; Fusion has already calculated it. G61 requests exact path control. The translator does not alter machine velocity/acceleration limits, overrides or HAL. G95 needs the actual spindle-speed feedback configured in LinuxCNC; CSS needs X0 at the spindle centre with the selected WCS/tool offsets.

Threading/tapping, cycles, secondary spindles, rotary/multi-axis motion, controller cutter compensation, manual NC, through-tool coolant and unknown executable properties are rejected. There is no fallback that turns unsupported instructions into ordinary moves.

## LinuxCNC file filter

Place the reviewed plan at `~/.config/LinuxCNCNext-NC/plan.json`, or set `NEXTNC_PLAN` to its absolute path before starting LinuxCNC. Add these entries to your **simulation configuration first**, using your real checkout path:

```ini
[FILTER]
PROGRAM_EXTENSION = .stpnc Next-NC lathe and mill toolpaths
stpnc = /home/you/LinuxCNCNext-NC/bin/nextnc-filter
```

Preserve existing FILTER entries. The wrapper writes only G-code to stdout, diagnostics to stderr, and exits nonzero on failure. It does not start LinuxCNC, connect to a controller or modify your INI. [Installation and operating limits](docs/linuxcnc.md) include tool changes, offsets, entry sequencing and restart considerations.

To use the same tool-table check in the filter, set `NEXTNC_TOOL_TABLE` to the existing table's absolute path before starting LinuxCNC. This is an input file reference, not a second set of tooling settings.

## Diagnostics

Inspect, plan-template, preflight, translate and the filter archive local JSON reports, including input/plan hashes, error code/context or validated program summary, and a generated-line-to-operation/path map for translations.

Inspect reports include per-operation feeds, spindle settings, entry/exit coordinates and continuation/connection eligibility. Translation reports also list the actual boundary decisions, link starts/ends/waypoints, source-line mapping and number of unchanged axis words omitted. A difference in CAM feed settings is preserved, not silently normalized to another file's feed.

The 0.4.0 source map gives explicit one-based G-code line numbers, operation line ranges, polyline segment/vertex numbers, motion starts/ends, arc centre offsets and source geometry, dwell positions and machine/work frames for transition waypoints. The map is bound to the candidate G-code SHA-256 and decoded program fingerprint. It describes generated commands, not execution progress, and does not enable run-from-line recovery.

- Linux: `${XDG_STATE_HOME:-~/.local/state}/LinuxCNCNext-NC/diagnostics/`
- Windows: `%LOCALAPPDATA%\LinuxCNCNext-NC\diagnostics\`
- Override: `NEXTNC_DIAGNOSTICS`

`latest.json` identifies the most recent run; `latest-error.json` retains the last failure even after success. Timestamped reports retain individual runs. These may contain private names/paths and coordinates in error context; nothing is uploaded. The CLI returns 0 for success, 1 for validation/file failure and 2 for usage errors.

## Development and validation

```sh
npm test
npm run example
npm run test:linuxcnc  # Linux: requires rs274 from linuxcnc-uspace
```

Tests cover units/CSS scaling, arc direction/full circles, per-revolution feeds, state changes, preserved vertices/repeated paths, exact numbers, mappings/transitions, rejection, input preservation, active-comment injection and diagnostic archives. CI runs on Windows/Linux with Node 20/22/24 and runs generated mm/inch samples through LinuxCNC's standalone `rs274` in an isolated Debian container. That interpreter does not connect to hardware. See [validation scope](docs/validation.md).

The first release is for development and simulation. Machine-specific clearance, tooling, WCS alignment, M6 behavior, feedback, abort/recovery and physical operation still require commissioning. Start generated programs from the beginning; arbitrary run-from-line/restart is not implemented or validated.

## License and provenance

MIT. The bounded reader/inspector and synthetic-fixture writer are vendored from Fusion360Next-NC v0.2.0 with their [license and pinned provenance](vendor/fusion360next-nc/PROVENANCE.md). LinuxCNC is a separate dependency. No Autodesk executable/source, private CAD/job data or physical-machine settings are distributed. [Primary references](docs/references.md).

## Architecture research update

See [the architecture review and resulting improvements](docs/architecture-research.md) for the three-paper review, stronger input checks and indexed interpretation. The subsequent [adapter review](docs/adapter-review-plan.md) informed the report and installation tools; v0.5.0 also adds a separate XYZ profile without changing legacy turning semantics.

The [NIST toolkit review](docs/nist-toolkit-review-plan.md) informed v0.6.0 command/state records and semantic verification. The [STEPNode review](docs/stepnode-review-plan.md) informed v0.7.0 source provenance, command validation and ordered output-completeness checks for both machines.

The [STEPNCpp review](docs/stepncpp-review-plan.md) informed v0.8.0's explicit process quantities, automatic job requirements, and systematic geometry/unit-equivalence checks for both machines. Reports distinguish validated translation and optional table snapshots from unverified controller commissioning; no new machining options are required.

The sixth-repository [Duy247 STEP-NC-Translator review](docs/duy-translator-review-plan.md) informed v0.9.0's final G-code audit and optional saved-file identity check. These close the demonstrated text-only audit gap and help distinguish an archived candidate from an edited or unrelated saved file, for both machines.

The [EfrainRodriguez/step-nc review](docs/efrain-step-nc-review-plan.md) informed v0.10.0's accepted-profile field checks, independent parser comparisons in CI, and explicit validation coverage. The parser tooling remains isolated from the dependency-free translator; general EXPRESS conformance is still unchecked.

The [Mastercam exporter review](docs/mastercam-stepnc-review-plan.md) informed v0.11.0's post-M6 state restoration, independent policy audit and process-transition tests. All 16 deliberately omitted commands accepted by the v0.10.0 probes are now rejected. The exporter and its external SDKs are not dependencies.

The [step-nc-adapters review and plan](docs/step-nc-adapters-review-plan.md) identifies comment/whitespace parsing gaps and proposes clearer profile compatibility diagnostics plus a small public API. These changes are planned, not implemented; APT/additive import and the Windows STEP Tools backend are outside the proposed scope.
