# XYZ milling and lathe compatibility

Use Fusion360Next-NC **v0.2.0 or newer** for milling. The CPS selects the profile from Fusion's section type; there is no user setting for feeds, depths, compensation, clearance or machine type. A single export must be either lathe or mill.

| Contract | Lathe | XYZ mill |
| --- | --- | --- |
| Profile | `next-nc/turning-toolpath/0.1` | `next-nc/milling-toolpath/0.1` |
| Coordinates | X radius, Y=0, Z | Cartesian X/Y/Z, fixed +Z tool axis |
| Initial plane | G18 | G17 |
| Native arcs | XZ | XY, XZ and YZ |
| Fusion logical offset | compensationOffset | lengthOffset |
| Spindle | RPM or CSS with cap | RPM |
| New plan template | execution-plan/3 | execution-plan/4, `machine: "mill"` |

Both use G8, absolute positions, incremental arc centers, Fusion-compensated paths, G40 and G61. Plane changes are emitted only when needed and reasserted after an M6. LinuxCNC's existing G43 H record supplies tool offsets; the translator neither calculates nor writes physical offsets. Feed modes and coolant state come from the file.

The mill profile accepts resolved linear/rapid XYZ toolpaths, dwell and principal-plane circular paths. Fusion linearizes milling helices at the tighter of its operation tolerance and the post's existing tolerance. Every returned vertex is retained. Bounded floating-point endpoint closure preserves the exact Fusion endpoint; larger discrepancies stop export. The translator receives polylines, not helical entities.

Rotary, indexed/tilted or multi-axis machining, mixed mill-turn programs, probing, threading/tapping, canned cycles, controller compensation and secondary spindles remain unsupported. A milling bore expressed as a helical toolpath is supported; a boring canned cycle is not. Choose operations/toolpaths that fit this scope; unsupported callbacks fail explicitly.

## Plans

Generate a fresh plan template for each new decoded program. Schema 4 requires a machine identifier matching the profile. Each machine retract, work-coordinate approach and program-end retract must establish all X/Y/Z coordinates using ordered, single-axis waypoints. The approach must reach the exact recorded entry point on all three axes. The translator cannot infer a safe Z plane or XY travel order from the input.

`continue` preserves state only at an exactly matching XYZ boundary. A reviewed `link` can use one or more single-axis work-coordinate moves, preserving unchanged axes from the known previous exit; it must reach the next entry exactly. Tool, offset, WCS and process-state checks still apply.

`examples/simulation-mill-plan.json` illustrates the syntax with synthetic clearances. Its numbers are **not machine settings**. Review your physical clearances and coordinate systems independently.

Existing lathe schemas 1, 2 and 3 retain their behavior and may not be used for a mill. Schema 4 can also explicitly declare `machine: "lathe"`. Legacy turning decoded models/fingerprints are unchanged. The baseline lathe G-code is unchanged except for the translator version banner.

This remains a bounded AP238 toolpath bridge, not support for arbitrary STEP-NC machining features or every ISO 14649 file. The machine profile is part of the program fingerprint.
