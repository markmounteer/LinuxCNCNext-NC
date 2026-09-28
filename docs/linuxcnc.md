# LinuxCNC integration

This implementation is an input filter. LinuxCNC opens `.stpnc`, invokes `bin/nextnc-filter` with the filename, then previews/executes the returned G-code using its existing interpreter. The translator never uses LinuxCNC command APIs, HAL, SSH or motion control.

Install Node.js 20+ on the LinuxCNC computer. Clone or extract the release to a permanent path, and ensure `bin/nextnc-filter` and `bin/nextnc.js` are executable (`chmod +x` if a ZIP extractor lost executable bits). No npm packages are needed. Run the synthetic example in an isolated simulation configuration first. Keep physical-machine configurations separate.

The filter plan defaults to `${XDG_CONFIG_HOME:-$HOME/.config}/LinuxCNCNext-NC/plan.json`; `NEXTNC_PLAN` overrides it. Quote environment paths in your launcher if they contain spaces. LinuxCNC's INI filter command should name the wrapper's absolute path. Preserve your existing `[FILTER]` extensions. If the checkout path contains spaces, use a simple launcher path without spaces or quote the filter command as supported by your GUI.

The same CLI also runs on Windows for inspection/translation. Copying an `.ngc` file to LinuxCNC does not bypass the need for a reviewed execution plan and machine acceptance.

## Machine requirements

- Single spindle 0, XZ axes, unrotated WCS and compatible physical X-radius/tool-reference conventions.
- Actual tool-table records matching the reviewed T/H mappings; M6 behavior and clearance verified for the machine.
- Correct WCS origin. CSS requires X0 at the spindle centre, including tool offsets.
- Actual spindle speed feedback connected for G95, and spindle-at-speed behavior configured as required by LinuxCNC. Successful offline interpretation does not establish this wiring.
- Reviewed entry/retract paths within machine travel and clear of stock, fixtures and tools. No G53, homing or machine maximum is inferred from a file.
- Controller limits, emergency stop, stop/abort and recovery commissioned independently.

Through-tool coolant and threading are unsupported. M7/M8/M9 preserve the exported flood/mist/off requests but require the corresponding machine outputs. A machine without coolant must use a CAM coolant setting consistent with its capabilities.

## Preview and restart

The output includes source operation comments and the local report maps each G-code line to its operation/path or transition. Shared STEP curves still generate every ordered move. Run from the beginning; run-from-line, block search and mid-operation restart have not been designed or tested. Translation cannot preserve controller state across an arbitrary restart point.

G61 conservatively preserves the path. Machine limits and trajectory planning still determine actual velocity. Schema 2 plans can avoid needless spindle stops and boundary travel where exact continuation is validated; this does not estimate cycle time or change blending/acceleration. Existing schema 1 plans retain their retracts.

G-code formatting expands exponent notation without truncating decoded coordinates. For G0/G1 body moves, an axis word is omitted only when its coordinate is exactly unchanged from the preceding decoded point. Every motion vertex and feed/state transition remains present. Arcs and full circles keep their explicit centre/sense format. CSS unit conversion is the only required scale conversion; toolpath units and feed units otherwise remain unchanged. No arithmetic loop recognition, geometric fitting or coordinate rounding is performed.
