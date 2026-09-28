# LinuxCNCNext-NC

[![Tests](https://github.com/markmounteer/LinuxCNCNext-NC/actions/workflows/ci.yml/badge.svg)](https://github.com/markmounteer/LinuxCNCNext-NC/actions/workflows/ci.yml)

An experimental translator and LinuxCNC input filter for XZ turning files from [Fusion360Next-NC](https://github.com/markmounteer/Fusion360Next-NC).

It reads the `next-nc/turning-toolpath/0.1` profile, validates it, and produces LinuxCNC RS274 G-code. LinuxCNC supplies preview, interpretation, trajectory planning, limits and HAL. This is a translation bridge, not a native STEP-NC interpreter or a general AP238 implementation. No LinuxCNC machine configuration or hardware is changed by installing this repository.

## Quick start

Requires Node.js 20 or later; there are no npm dependencies. LinuxCNC is needed only for preview/execution and the optional standalone interpreter tests.

```sh
git clone https://github.com/markmounteer/LinuxCNCNext-NC.git
cd LinuxCNCNext-NC
npm test
node bin/nextnc.js inspect examples/synthetic.stpnc
node bin/nextnc.js translate examples/synthetic.stpnc --plan examples/simulation-plan.json --output /tmp/nextnc-simulation.ngc
```

The example coordinates and tool table assumptions are **for simulation only**. They are not settings for a Grizzly or any other physical lathe.

## Translate your own export

```sh
node bin/nextnc.js inspect /path/1001.stpnc
node bin/nextnc.js plan-template /path/1001.stpnc --output /path/1001-plan.json
# Fill and review the template's mappings and transition paths.
node bin/nextnc.js translate /path/1001.stpnc --plan /path/1001-plan.json --output /path/1001.ngc
```

The template deliberately has `null` mappings and paths. The export contains each operation's entry point, but no machine-safe approach, tool-change or inter-operation retract. Translation requires explicit LinuxCNC tool/H-offset and work-offset mappings, plus ordered machine-coordinate retract and work-coordinate approach waypoints. It never assumes work offset 0 means the currently active WCS or that a straight rapid between operations is safe.

An execution plan is bound to the exact decoded program fingerprint. A change to tooling, units, offsets, paths, feeds or process states requires a reviewed plan for the new program. Export timestamps, record numbering and equivalent geometry sharing do not invalidate it. See [execution plans](docs/execution-plan.md).

The entire input and plan are checked before any G-code reaches stdout or an output file. Existing files are never overwritten. The final output path is created atomically; failed writes do not leave a partial program there.

## Supported translation

| Next-NC content | LinuxCNC output |
| --- | --- |
| Millimetres / inches, X radius, XZ plane | G21 / G20, G8, G18 |
| Absolute positions, relative arc centres | G90, G91.1 |
| Rapid/cutting polylines | Every vertex in order as G0/G1 |
| CW/CCW arcs and full circles | G2/G3 with I/K; full circles omit endpoint axes |
| Feed per minute / revolution | G94 / G95 with explicit F on every feed-mode change |
| Constant RPM | G97 S, M3/M4 on spindle 0 |
| CSS and maximum RPM | G96 S D; mm/min converted to m/min, inch/min to ft/min |
| Off / flood / mist coolant | M9 / M8 / M7 |
| Dwell | G4 P in seconds |
| Reviewed logical tool/offset mapping | Tn M6, G43 Hn; no repeated M6 for the same mapped tool |
| Reviewed work-offset mapping | G54 through G59.3 |

G40 prevents a second application of nose-radius compensation; Fusion has already calculated it. G61 requests exact path control. The translator does not alter machine velocity/acceleration limits, overrides or HAL. G95 needs the actual spindle-speed feedback configured in LinuxCNC; CSS needs X0 at the spindle centre with the selected WCS/tool offsets.

Threading/tapping, cycles, secondary spindles, Y/multi-axis motion, controller cutter compensation, manual NC, through-tool coolant and unknown executable properties are rejected. There is no fallback that turns unsupported instructions into ordinary moves.

## LinuxCNC file filter

Place the reviewed plan at `~/.config/LinuxCNCNext-NC/plan.json`, or set `NEXTNC_PLAN` to its absolute path before starting LinuxCNC. Add these entries to your **simulation configuration first**, using your real checkout path:

```ini
[FILTER]
PROGRAM_EXTENSION = .stpnc Next-NC turning toolpaths
stpnc = /home/you/LinuxCNCNext-NC/bin/nextnc-filter
```

Preserve existing FILTER entries. The wrapper writes only G-code to stdout, diagnostics to stderr, and exits nonzero on failure. It does not start LinuxCNC, connect to a controller or modify your INI. [Installation and operating limits](docs/linuxcnc.md) include tool changes, offsets, entry sequencing and restart considerations.

## Diagnostics

Every command archives a local JSON report, including input/plan hashes, error code/context or validated program summary, and a generated-line-to-operation/path map for translations.

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

MIT. The bounded reader/inspector and synthetic-fixture writer are vendored from Fusion360Next-NC v0.1.5 with their [license and pinned provenance](vendor/fusion360next-nc/PROVENANCE.md). LinuxCNC is a separate dependency. No Autodesk executable/source, private CAD/job data or physical-machine settings are distributed. [Primary references](docs/references.md).
