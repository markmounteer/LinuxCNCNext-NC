# Offline preflight and traceability

`preflight` validates and generates the complete candidate program in memory, then returns JSON. It uses the same reader, plan validation and translation path as `translate`, preventing the two checks from drifting. No G-code is written by this command. A valid reviewed execution plan remains required, and every translation repeats validation.

```sh
node bin/nextnc.js preflight job.stpnc --plan reviewed-plan.json --tool-table /path/tool.tbl --output new-review.json
node bin/nextnc.js translate job.stpnc --plan reviewed-plan.json --tool-table /path/tool.tbl --output new-job.ngc
```

The JSON contains `status`, `gcodeWritten: false`, the inspection report and source map. Normal local diagnostic archives also retain input, plan and optional tool-table paths/hashes. The existing no-overwrite and no-partial-output guarantees apply. Reports can contain private operation names and coordinates.

## Tool-table scope

The optional UTF-8 snapshot is limited to 1 MiB. The importer accepts whitespace-separated LinuxCNC `T P X Y Z A B C U V W D I J Q` words, blank lines and semicolon comments. T/P are required unsigned integer words; Q, when present, is an integer from 0 through 9. Duplicate words, duplicate tool numbers, unknown words and nonfinite numeric data fail with a source line number. Other optional fields may be absent. T0/P0 are allowed in the snapshot but execution-plan mappings still require positive tool/H records.

For each operation, both the selected T record and independent G43 H record must exist. They need not be the same number. Missing records are aggregated across operations. Measurements are not compared with program units: the table is in machine units, and this check only reads record identities and syntax. Pocket uniqueness, changer topology and pocket range depend on controller configuration and are not checked here.

The existing LinuxCNC configuration remains authoritative. The supplied snapshot may differ from LinuxCNC's loaded table or `[EMCIO]DB_PROGRAM` tool database. The checker does not verify physical tool geometry or condition, measured offsets, stock/fixtures, WCS alignment, machine travel, spindle feedback, INI/HAL, M6 behavior or clearance. A pass means only that the reported offline checks passed. Without a snapshot, `inspection.toolTable.status` is `not_checked`.

## Source-map contract

`inspection.traceability.schema` is `linuxcnc-next-nc/source-map/1`. Each `sourceMap` entry corresponds to exactly one emitted G-code line and includes its one-based `line`. Operation and path numbers identify positions in this decoded program, not permanent Fusion IDs across reposts. `inspection.programFingerprint` binds their meaning to the decoded input; `traceability.gcodeSHA256` identifies the exact candidate G-code bytes, including the release banner and terminal newline. An edited output needs a fresh map.

Body motions have `phase: toolpath`, `action: motion`, section/operation/path and a motion kind. Linear/rapid entries add one-based segment and from/to vertex indices within the decoded polyline; compaction and shared curves therefore do not hide individual moves. Coordinates are XYZ in program units, with radius X. Starts track the commanded position, including the small join discrepancy the reader may accept; they are not measured positions.

Arcs include commanded start/end, relative `centerOffset` corresponding to I/K, original `sourceStart`/`sourceCenter`, direction and full-circle state. Original source geometry is kept distinct from commanded coordinates at tolerance-accepted joins. Dwells record seconds and the current commanded position.

Transition entries identify retract, approach, reviewed link or program end, one-based waypoint and `motion.frame` (`machine` for G53, otherwise `work`). Their target contains only the explicitly commanded axis. Unknown initial machine positions and transformations between machine/work frames are not invented. Modal/tool/spindle/feed/coolant lines are identified as state actions; comments share that non-motion classification. `operationRanges` includes transition/state lines belonging to each operation and its count of body motion blocks; program-end moves are separate.

These records support diagnosis and comparison. They neither monitor execution nor provide restart, tool-breakage recovery or automatic replacement-tool selection.

## Research rationale

Suh et al., [STEP-compliant CNC system for turning: Data model, architecture, and implementation](https://doi.org/10.1016/j.cad.2006.02.006), *Computer-Aided Design* 38 (2006), 677–688, separates authoring, machine adaptation/verification and execution (pp. 681–685; Figs. 7, 16–17). Its execution layer relates generated control code to workingsteps (pp. 685–686). This implementation adopts bounded offline checks and diagnostic traceability from those ideas. Fusion continues to plan cutting paths; LinuxCNC continues to interpret and control motion. Full feature/stock models, multi-turret scheduling and autonomous recovery described in the paper are outside this toolpath bridge.

Tool-table syntax and configuration ownership follow the [LinuxCNC tool compensation reference](https://www.linuxcnc.org/docs/stable/html/gcode/tool-compensation.html).
