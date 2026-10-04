# Native source and capability contract

The producer emits revision 2; Rust reads revisions 1 and 2. The retained
JavaScript G-code translator reads revision 1 and rejects revision 2, including
revision-2 properties under an old profile label. There is no native execution
adapter in this checkpoint. Passing offline checks never advertises installed
controller support.

| Intent | Fusion revision 2 | Rust source/preflight | Shared contract | Native execution admission |
| --- | --- | --- | --- | --- |
| Lathe XZ, radius X; mill XYZ | Explicit profile and coordinates | Checks machine, axes, unit graph, ordered geometry | Machine and PointMm; v2 enclosing frame remains explicit | Requires fresh task-owned coordinate binding |
| Rapid/linear | Ordered vertices and process state | Retains each vertex use, including repeats | v2 Geometry::Line, Feed::Rapid/PerSecond | Adapter pending |
| Principal-plane circular path | Exact endpoints, center, direction, total sweep | Independent continuous geometry check in mm | v2 Geometry::Circular | Adapter pending |
| Constant-radius helix | Analytic path, signed axial rise | Separate `helix` requirement; no linearization | v2 geometry and separate helix capability | Adapter pending |
| More than one turn | Positive total radians | Separate `multiple-turns` requirement, 10000-turn source bound | Separate multi-turn capability | Adapter pending |
| CAM tolerance | Positive source value and provenance, or explicit missing | Retained; never used as an implicit extra allowance | v2 Tolerance | Fit/blend budget and admission remain separate |
| Movement purpose | Explicit class, unspecified if absent | Validated against rapid/cutting mode; ordered changes retained | v2 Movement | Cannot erase purpose boundaries |
| Feed per revolution | Dimensioned source length/revolution | Preserved; complete job requires synchronization capability | Feed::PerRevolution | No nominal-RPM substitution; Stage 4 owns implementation |
| Constant surface speed | Lathe only, surface speed and mandatory RPM cap | Preserved; separate `css` requirement | Spindle::Css | Runtime CSS/coordinate support required |
| Spindle RPM/direction | Explicit state per ordered use | Retained independently from geometry | Spindle::Rpm/Stop | Task-owned event and at-speed handling pending |
| Flood/mist/off | Explicit functions | No inferred replacement; each state retained | Coolant | Task-owned mapping pending |
| Through-tool coolant | Actionable precheck rejection | `COOLANT_CAPABILITY` rejection | No implemented mapping | Unsupported |
| Tool change and independent offset | Logical source tool and offset | Reviewed mappings and optional file-table checks | ChangeTool and ToolOffset | Physical completion and fresh binding required |
| Dwell/completion | Positive dwell, ordered workplan | Dwell retained; completion requirement explicit | Dwell/Fence/End | Drain/completion implementation pending |
| Phase threading, tapping, probing, rotary, unknown executable properties | Unsupported operations rejected | Closed vocabulary, strict graph/shape/units; declared requirements must equal derived requirements | Not representable by current source contract | Rejected; never substituted |

The semantic model records the source initial process state and state at every
ordered toolpath use. Geometry, spindle, feed mode/value and coolant are compared
as a complete ordered model, not just endpoints or bounds. Revision 1 supplies
no CAM tolerance or movement provenance; the native compiler must represent
these as missing/unspecified, not invent values.

## Offline target manifest

`preflight input.stpnc plan.json --target capabilities.json` accepts a closed
`nextnc-native/target-capabilities/1` object containing `machine`,
`motionContractVersion: 2`, `capabilities`, and `evidenceSha256`. Capabilities use
the names in `src-rust/capabilities.rs`; duplicates/unknown values fail, and
helix/multiple-turns also require planar-arc. Evidence is a lowercase SHA-256
identity supplied by the machine package owner. The compiler does not fetch,
authenticate or infer installed support from that digest.

The complete decoded job is checked before any result is returned. Missing
support produces `UNSUPPORTED_CAPABILITIES` with the complete missing list. An
omitted manifest is explicitly `not_checked`. This gate covers source
requirements; later compiler policy can add requirements such as at-speed or
blend. Neither a manifest nor a cached result grants live permission to run.

## Reproducible producer/consumer evidence

- `tests-rust/fixtures/native/manifest.json`: producer revision and seven source
  hashes, 120 synthetic profiles and ten outputs from Autodesk post engine
  5.413.5. Native post evidence includes lathe face/profile, mill face/bore/tool
  change, four expanded drilling cycles, and positive dwell.
- `cargo test --locked --test profile`: exact ordered model and fingerprint
  comparisons for all 130 native profiles and four legacy mill/lathe mm/inch
  profiles; malformed extensions and missing target capabilities fail.
- `cargo test --locked --test geometry`: independent continuous path enclosure,
  length, sweep, direction, plane and rise tests in canonical mm.
- `node scripts/check-native-posting.js ../Fusion360Next-NC`: ten actual posted
  programs compare exactly, and the retained revision-1 consumer rejects each.
  This is an optional development comparison; Rust tests use committed data.

Raw source decimals use round-trip IEEE-754 parsing. Revision-2 fingerprints hash
only source circular fields in a fixed order, excluding a computed radius whose
last bit can vary across math libraries. Revision-1 declared radius remains in
its unchanged fingerprint. The numeric radius/sweep consistency floor follows
the source unit convention and is converted to mm for checking; it is not an
optimization or manufacturing error allowance.
