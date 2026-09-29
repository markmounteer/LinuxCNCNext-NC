"use strict";
// This describes a validated job. It neither configures nor queries LinuxCNC.
function jobRequirements(program, process, sourceMap, ranges, transitions, toolTable) {
  const machine = program.model.machine || "lathe", quantities = new Map();
  const modes = new Set(), directions = new Set(), feeds = new Set(), coolant = new Set();
  for (const op of process.operations) for (const phase of op.phases) for (const q of phase.quantities) {
    if (q.kind === "spindleSpeed") modes.add(q.mode);
    if (q.kind === "spindleDirection") directions.add(q.source.value);
    if (q.kind === "feed") feeds.add(q.mode);
    if (q.kind === "coolant") coolant.add(q.source.value);
    if (!["spindleSpeed", "maximumRPM", "feed"].includes(q.kind)) continue;
    const key = `${q.kind}:${q.mode || ""}:${q.source.unit}`;
    if (!quantities.has(key)) quantities.set(key, {kind: q.kind, ...(q.mode ? {mode: q.mode} : {}),
      source: {unit: q.source.unit, min: q.source.value, max: q.source.value},
      output: {unit: q.output.unit, min: null, max: null}, sections: new Set()});
    const range = quantities.get(key); range.sections.add(op.section);
    range.source.min = Math.min(range.source.min, q.source.value); range.source.max = Math.max(range.source.max, q.source.value);
    if (q.output.value != null) {
      range.output.min = range.output.min == null ? q.output.value : Math.min(range.output.min, q.output.value);
      range.output.max = range.output.max == null ? q.output.value : Math.max(range.output.max, q.output.value);
    }
  }
  const unique = values => [...new Set(values)];
  const controllerChecks = [
    {id: "tool-change", detail: "Commission M6/remaps and the physical tool changer for the mapped tools."},
    {id: "offsets", detail: "Verify physical tool identity and calibrated T/H and work offsets."},
    {id: "travel-clearance", detail: "Verify machine travel, workholding and all cutting/approach/link/retract clearances."},
    {id: "spindle", detail: "Verify spindle 0, direction, attainable speeds and site at-speed behavior."},
    {id: "coolant", detail: "Verify coolant command behavior and wiring, including off at shutdown."}
  ];
  if (feeds.has("perRevolution")) controllerChecks.push({id: "spindle-feedback", detail: "G95 requires actual spindle 0 speed feedback (spindle.0.speed-in)."});
  if (modes.has("css")) controllerChecks.push({id: "css-origin", detail: "G96 requires X0 at the spindle centre with the selected work and tool offsets; the RPM cap is not actual speed feedback."});
  return {schema: "linuxcnc-next-nc/job-requirements/1", gcodeSHA256: process.gcodeSHA256,
    machine, axes: machine === "mill" ? ["X", "Y", "Z"] : ["X", "Z"], units: program.model.units, coordinates: process.coordinates,
    arcPlanes: unique(sourceMap.filter(e => e.command.type === "arc").map(e => e.command.plane)),
    spindle: {number: 0, modes: [...modes], directions: [...directions]}, feedModes: [...feeds],
    coolant: {requested: [...coolant], commands: unique(sourceMap.filter(e => e.command.type === "coolant").map(e => e.command.value))},
    quantityRanges: [...quantities.values()].map(r => ({...r, sections: [...r.sections]})),
    mappings: ranges.map(r => ({section: r.section, operation: r.operation, fusionTool: r.tool.number, fusionOffset: r.tool.offset, tool: r.mappedTool.tool, offset: r.mappedTool.offset, workOffset: r.mappedWorkOffset})),
    boundaries: Object.fromEntries(["continue", "link", "retract"].map(mode => [mode, transitions.filter(t => t.mode === mode).length])),
    evidence: {translation: {status: "passed", scope: "Supported profile, reviewed plan, command contract, ordered completeness, independent policy state and final-text checks."},
      toolTable, controller: {status: "not_checked", checks: controllerChecks.map(c => ({...c, status: "not_checked"}))}},
    limitations: ["Requested ranges exclude translator initialization and shutdown; they are not measured motion or spindle history.", "CSS and its RPM cap do not predict actual spindle speed.", "No HAL, INI, live controller or physical clearance checks were performed."]};
}
module.exports = {jobRequirements};
