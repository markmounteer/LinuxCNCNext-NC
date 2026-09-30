# Offline toolpath and stock review (v0.12.0)

Keep designing and generating toolpaths in Fusion. The translator's report now
lets you inspect those paths and their LinuxCNC output without installing
FreeCAD or changing controller settings. No review control edits G-code, feeds,
speeds, compensation, work offsets or the execution plan.

## Open a job review

Use the timestamped diagnostic JSON saved by a successful `preflight`,
`translate` or LinuxCNC filter run. The location is printed on stderr; see
[diagnostics](../README.md#diagnostics). Preflight's stdout JSON is the inspection, not
the diagnostic archive: the report command needs the latter.

```sh
node bin/nextnc.js report /path/job-diagnostic.json --gcode /path/job.ngc --output /path/new-review.html
```

Open the HTML locally in a browser. It is self-contained and requests no network
assets. The optional `--gcode` compares exact saved-file bytes with the archived
candidate hash. All displayed geometry still describes the archived candidate,
including when the selected file does not match. Archives are not authenticated
and a matching hash does not establish what LinuxCNC has loaded or executed.

The viewer supports lathe XZ (X radius) and fixed XYZ mill paths in mm/inch.
Select a line, move the slider, use previous/next, or click a displayed segment.
The selected segment is orange and its endpoint is marked. The nearby line list
shows the actual emitted text for new archives; older typed source maps show
their recorded command instead. STEP record/line references and execution-plan
JSON pointers remain visible with the tool, H offset, feed, spindle and coolant
state. A state-only or unresolved line can be selected without showing a segment.

Operation, projection and rapid/cut filters only change the view. Frame selection
keeps G53 machine coordinates and each G54..G59.3 work frame separate. There is
no invented transform between them. Z is horizontal and X vertical in the XZ
view. XYZ isometric is an orthographic projection, not a rotatable solid model.
Line playback advances at a fixed display rate, not at machine speed.

Arcs use the right-handed XY, ZX and YZ bases of G17/G18/G19, retaining full
circles without endpoint words. The drawing samples arcs into short segments;
distance metrics use radius and sweep, not the drawing tessellation. The viewer
accepts up to 200,000 source-map lines and displays up to 200,000 vertices per
view. A display cap is explicitly reported; it does not cap translation or its
motion totals. Unknown endpoints are omitted from the drawing and counted.

## Motion metrics

Successful translation/preflight reports contain additive
`linuxcnc-next-nc/motion-summary/1` data. HTML also computes it for usable older
typed maps. Totals and operation summaries report:

- Known cutting and rapid distance in program units, including reviewed links
  and approaches where their commanded axis differences are known.
- Motion blocks with unknown distance. A one-axis move can have a known length
  but remain undrawable because its other coordinates are unknown.
- Programmed dwell seconds, separately from feed time.
- Ideal feed seconds: `60 * distance / feed` for G94;
  `60 * distance / (feed * RPM)` for G95 with running constant-RPM spindle.
- Cutting blocks whose feed time cannot be estimated, including G95 with CSS.

Ideal feed seconds are a subtotal for resolved cutting blocks, **not total cycle
time or a guaranteed real-time bound**. No estimates are invented for rapids,
acceleration, feed overrides, spindle delays, tool changes or controller/remap
behavior. Known distances in separate frames can be summed as scalar lengths;
their points are never combined into one path.

## Optional stock-removal preview

The current profiles do not include a complete stock, cutter, holder and fixture
model. A separate review-only JSON supplies missing **physical preview geometry**.
These fields do not override Fusion machining choices or LinuxCNC settings.
This first model is limited to an axis-aligned stock block, vertical flat-end
cutters, fixed XYZ milling and one explicit work offset. It is not a lathe-stock,
ball-nose, undercut, fixture, holder or machine collision simulator.

```json
{
  "schema": "linuxcnc-next-nc/stock-setup/1",
  "machine": "mill",
  "units": "mm",
  "programFingerprint": "COPY inspection.programFingerprint.value FROM THE ARCHIVE",
  "gcodeSHA256": "COPY inspection.traceability.gcodeSHA256 FROM THE ARCHIVE",
  "workOffset": "G54",
  "stock": {"min": [-5, -3, -3], "max": [5, 3, 0]},
  "tools": {"1": {"shape": "flat-end", "diameter": 2, "cuttingLength": 8}},
  "resolution": 0.125
}
```

The coordinates above are **synthetic examples**, not settings for a real job.
Supply stock bounds and actual cutter geometry in the program's units and work
frame. Tool keys are the mapped LinuxCNC **T numbers**, not H offsets or Fusion
tool IDs. The toolpath point represents the cutter's bottom centre; cutting
length extends in +Z. Dimensions must be positive, bounds must increase and
both hashes must match the candidate. Regenerating a different candidate,
including after a translator version change, requires a newly bound setup.
Do not invent a stock placement or substitute a different cutter shape.

```sh
node bin/nextnc.js report /path/job-diagnostic.json --gcode /path/job.ngc --stock-setup /path/stock.json --output /path/new-stock-review.html
```

The report shows the **final** top surface and approximate removed/remaining
volume. It does not animate stock as the selected line changes. Cells are sampled
at their centres, and tool positions at intervals no greater than half a cell.
This can miss narrow or tangential features: resolution is not a machining
tolerance or an error guarantee. Rapids never remove material; sampled rapid
intersections produce line-linked warnings. G53 moves and moves with unknown
positions are explicitly excluded. Absence of a warning is not proof of clearance.

Missing tools, other work offsets, incompatible units/identities, unsupported
shapes and buried cuts that the top-surface model cannot represent fail the
preview before writing an HTML file. Limits are 1 MiB for the setup JSON,
512 cells per grid axis, 200,000 path samples and ten million cell checks.
Tool diameter must span at least two requested cells. Increasing the numerical
resolution value makes the grid coarser. These bounds affect only the optional
review. They never reject or alter an otherwise valid translation.

The archived job, G-code, plan and controller tool table remain unchanged.
The setup hash in HTML covers `JSON.stringify` of the parsed setup object, not
its original whitespace. Only a new report file is written; errors and repeated
output filenames leave existing reports intact.

## Demonstrations and implementation provenance

```sh
npm run example:review
```

This generates synthetic `mill-review.html`, `lathe-review.html`, the mill STEP,
G-code, execution plan, stock setup and both archives under
`artifacts/review-examples/`. Re-running this example generator replaces only
those synthetic example files. Normal CLI output remains exclusive.

The FreeCAD review used main commit
[`5591937592c863c59a7b3497fbe6ee06ff4b440e`](https://github.com/FreeCAD/FreeCAD/tree/5591937592c863c59a7b3497fbe6ee06ff4b440e):
the [CAM inspector](https://github.com/FreeCAD/FreeCAD/blob/5591937592c863c59a7b3497fbe6ee06ff4b440e/src/Mod/CAM/Path/Main/Gui/Inspect.py),
[geometry tests](https://github.com/FreeCAD/FreeCAD/blob/5591937592c863c59a7b3497fbe6ee06ff4b440e/src/Mod/CAM/CAMTests/TestPathGeom.py),
and [simulator parser](https://github.com/FreeCAD/FreeCAD/blob/5591937592c863c59a7b3497fbe6ee06ff4b440e/src/Mod/CAM/PathSimulator/AppGL/GCodeParser.cpp).
The useful concepts are command-linked inspection, independent geometry cases
and explicit stock/tool geometry. Our implementation and test expectations are
original JavaScript; no FreeCAD source was copied, no FreeCAD runtime or license
dependency was added, and its simulator was not used to interpret LinuxCNC code.

Browser data is escaped in a non-executable template; a CSP permits only the
hash of the bundled viewer script. Archive strings enter dynamic UI through
`textContent`. Old failure archives remain readable without JavaScript.
