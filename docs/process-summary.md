# Process summaries and job requirements

Preflight, translation and filter diagnostic archives include two automatic
report additions for XZ lathe and fixed-axis XYZ milling jobs. There are no new
options, input files or machining settings. `inspect` still describes the source
without a reviewed execution plan; `doctor` still checks installation only.

## Process summary

`inspection.processSummary` has schema `linuxcnc-next-nc/process-summary/1` and
the same candidate `gcodeSHA256` as the source map. It contains each operation's
initial settings and every decoded path in order, including rapid and dwell.

Each quantity records its kind/mode, decoded source value and unit, STEP property
provenance, output value and unit, conversion explanation, and one-based emitted
line numbers. `emitted` means those lines issued a command; `inherited` means no
new command was needed and the known commanded value was retained. `unknown`
preserves uncertainty in the command-state model. No missing value becomes zero.

For example, a lathe CSS request of 80,000 mm/min becomes LinuxCNC `G96 S80`,
meaning 80 m/min. An inch request of 1,200 in/min becomes `S100`, or 100 ft/min.
RPM, feed per minute, feed per revolution and dwell seconds are distinguished.
Spindle direction is separate from its nonnegative speed magnitude; X is radius
on the lathe. These are the current profile's decoded values, not raw signed
STEP tokens or measured machine values.

The phase's `commanded` state and `commandedUnits` also show retained feed on
rapid/dwell and explicit unknown state. Context first/last lines bound the
initial transition or path; a quantity's `lines` identify its exact commands.
Reversal stop/start and coolant off/on sequences are included in these bindings.

`policyCommands` lists non-motion initialization, plan mapping, transition and
shutdown instructions separately. It includes their actual G-code, provenance,
dimensioned quantities where present, and state invalidated by M6 or program
end. A mode-only final `G94` has no invented F quantity. Source-requested spindle
speeds never include policy shutdown `S0` in their ranges.

An M6 remap may change coolant or other state. If the translator has not emitted
a command establishing a value afterward, the report retains `unknown` even
when the source requests off. This documents the existing commissioning limit;
it does not infer what the remap did or change generation behavior.

## Job requirements

`inspection.jobRequirements` has schema `linuxcnc-next-nc/job-requirements/1`.
It reports axes, units, radius/Cartesian convention, arc planes, spindle 0 modes
and directions, feed modes, coolant requests/commands, dimensioned parameter
ranges, per-operation T/H/WCS mappings and reviewed boundary counts. All paths
contribute, including a later switch to G95 or a different spindle/coolant state.

Evidence remains explicitly separated:

- Translation checks passed: supported profile, plan, command contract and
  ordered completeness checks ran before this summary was built.
- Tool table: the existing optional snapshot result is carried through exactly.
  A successful snapshot check is not a live-table or physical-tool verification.
- Controller commissioning: always `not_checked`. G95 adds the actual spindle
  speed-feedback requirement; CSS adds the X0/offset prerequisite. M6, physical
  tooling, offsets, wiring, travel and clearance remain commissioning matters.

Ranges describe requested settings, not achieved speed, elapsed machining time
or collision clearance. CSS plus its cap does not predict the actual RPM history.
The report never reads or duplicates LinuxCNC INI/HAL settings.

## HTML and compatibility

The existing `report` command renders readable quantity and requirement tables,
alongside detailed source records. Source text is escaped and the report has no
scripts or network assets. Older archives display `Not recorded` for these
summaries; original source quantities are not reconstructed from output alone.

Both additions are outside the fingerprinted model. Profile versions, execution
plan schemas 1–4, the source-map schema and existing report fields are unchanged.
The reporting modules consume the audited output and never supply generation
values. Existing golden tests require unchanged G-code after the release banner
and unchanged program fingerprints.
