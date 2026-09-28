# Architecture research applied in 0.4.1

This release strengthens the adapter boundary without changing generated motion. It uses the updated Fusion360Next-NC reader/inspector, parses once, and retains record, source-line and source-column context in archived errors.

The [upstream research note](https://github.com/markmounteer/Fusion360Next-NC/blob/main/docs/architecture-research.md) gives page-specific findings from Liu et al. (2006), Minhat et al. (2009) and Álvares et al. (2020), DOI links, implementation decisions and a bounded inspection benchmark. Full papers and private machining inputs remain local.

The papers support separating STEP interpretation and process data from the controller. They do not establish that LinuxCNC directly executes arbitrary AP238, that a toolpath-only document provides collision geometry, or that a robot machining demonstration validates a lathe. The robotic study reports significant geometric/dimensional errors in its physical trials. Minhat's prototype manually built its function-block applications; automatic STEP-NC conversion was future work.

## Changes and checks

- Parse the actual Part 21 header. Reject forged/duplicate schema records and malformed header attributes before G-code output.
- Reject disconnected technology/functions records, detached execution relationships and malformed tool associations in the bounded Next-NC execution graph.
- Index entity types and relationships; reuse the parsed document for strict profile validation and ordered decoding.
- Preserve parser/inspector diagnostic context in `latest-error.json`. Existing all-or-nothing CLI/filter behavior remains in place.
- Regression comparison against 0.4.0 covers six synthetic programs (metric/inch, ordinary/retract, continuation and reviewed link) and a private seven-operation export. G-code is identical except for the release comment, and source maps are identical. Private files are not distributed.

The LinuxCNC filter still emits RS274 G-code for LinuxCNC to interpret. It does not take over trajectory planning, servo timing or hardware control. [LinuxCNC's current filter documentation](https://linuxcnc.org/docs/stable/html/gui/filter-programs.html) describes this integration mechanism. The format, machine-plan schema, options and geometric precision are unchanged.

Native Autodesk sample posting and isolated LinuxCNC interpreter tests are software checks. User-job reposting through Fusion, actual tool offsets, stock/fixture clearance and physical execution still require validation on the intended setup.
