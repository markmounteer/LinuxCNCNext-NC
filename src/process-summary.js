"use strict";
// Reporting only: consume the audited map, never feed report values back into
// generation. State is commanded state, not controller feedback or M6 internals.
function processSummary(program, sourceMap, boundaries, lines, gcodeSHA256) {
  const length = program.model.units === "mm" ? "mm" : "in";
  const speedUnit = mode => mode === "css" ? (length === "mm" ? "m/min" : "ft/min") : "RPM";
  const feedUnit = mode => length + (mode === "perRevolution" ? "/rev" : "/min");
  const groups = new Map(), initialStates = new Map();
  const initialAt = new Map(boundaries.filter(b => b.type === "path-start" && b.path === 1).map(b => [b.at, b.section]));
  const state = {};
  for (const entry of sourceMap) {
    if (initialAt.has(entry.line - 1)) initialStates.set(initialAt.get(entry.line - 1), {...state});
    for (const [key, change] of Object.entries(entry.stateChange)) state[key] = change.after;
    const key = `${entry.section}:${entry.path || 0}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(entry);
  }
  function phase(section, path, spec, provenance, snapshot, context) {
    const entries = groups.get(`${section}:${path || 0}`) || [];
    const sourceCommands = entries.filter(e => e.provenance.origin === "step");
    const quantities = [];
    function add(kind, sourceValue, sourceUnit, outputValue, outputUnit, property, types, conversion, mode) {
      const emitted = sourceCommands.filter(e => types.includes(e.command.type));
      quantities.push({kind, ...(mode ? {mode} : {}), source: {value: sourceValue, unit: sourceUnit, provenance: property},
        output: {value: outputValue ?? null, unit: outputUnit}, conversion,
        disposition: emitted.length ? "emitted" : outputValue == null ? "unknown" : "inherited",
        lines: emitted.map(e => e.line)});
    }
    const s = spec.spindle, p = provenance.process;
    add("spindleSpeed", s.speed, s.mode === "css" ? length + "/min" : "RPM", snapshot.spindleSpeed,
      speedUnit(snapshot.spindleMode || s.mode), p.spindle, ["spindleMode"],
      s.mode === "css" ? (length === "mm" ? "mm/min divided by 1000 gives m/min" : "in/min divided by 12 gives ft/min") : "RPM unchanged", s.mode);
    add("spindleDirection", s.clockwise ? "clockwise" : "counterclockwise", null, snapshot.spindleDirection, null, p.spindle, ["spindleStop", "spindleStart"], "Direction is separate from speed magnitude; reversal stop/start lines are included");
    if (s.mode === "css") add("maximumRPM", s.maximumRPM, "RPM", snapshot.spindleMaximumRPM, "RPM", p.spindle, ["spindleMode"], "RPM cap unchanged");
    add("coolant", spec.coolant, null, snapshot.coolant, null, {enabled: p.coolant, type: p.coolantType}, ["coolant"], "Off / flood / mist");
    if (spec.feed) add("feed", spec.feed.value, feedUnit(spec.feed.mode), snapshot.feedRate, feedUnit(snapshot.feedMode || spec.feed.mode), p.feed, ["feed"], "Feed units and mode preserved", spec.feed.mode);
    if (spec.kind === "dwell") add("dwell", spec.seconds, "s", spec.seconds, "s", provenance.dwell, ["dwell"], "Seconds unchanged");
    return {phase: path ? "path" : "initial", ...(path ? {path, kind: spec.kind} : {}),
      source: context, firstLine: entries[0]?.line, lastLine: entries.at(-1)?.line, quantities,
      // Includes the retained feed on rapid/dwell and explicit unknown values.
      commanded: Object.fromEntries(["spindleMode", "spindleSpeed", "spindleDirection", "spindleMaximumRPM", "coolant", "feedMode", "feedRate"].map(k => [k, snapshot[k] ?? null])),
      commandedUnits: {spindleSpeed: snapshot.spindleMode == null ? null : speedUnit(snapshot.spindleMode), spindleMaximumRPM: "RPM", feedRate: snapshot.feedMode == null ? null : feedUnit(snapshot.feedMode)}};
  }
  const operations = program.model.sections.map((s, i) => {
    const source = program.provenance.sections[i], section = i + 1;
    const context = {workingstep: source.workingstep, operation: source.operation};
    return {section, operation: s.name, phases: [phase(section, null, {spindle: s.initialSpindle, coolant: s.initialCoolant}, source, initialStates.get(section), context),
      ...s.paths.map((p, j) => {
        const entries = groups.get(`${section}:${j + 1}`);
        const snapshot = entries.find(e => e.modalState).modalState;
        return phase(section, j + 1, p, source.paths[j], snapshot, {...context, toolpath: source.paths[j].toolpath});
      })]};
  });
  const policyCommands = sourceMap.filter(e => e.provenance.origin !== "step" && !["comment", "rapid", "linear", "arc"].includes(e.command.type)).map(e => {
    const c = e.command, quantities = [];
    if (c.type === "spindleMode") quantities.push({kind: "spindleSpeed", value: c.speed, unit: speedUnit(c.mode)});
    if (c.type === "feed" && c.value !== undefined) quantities.push({kind: "feed", value: c.value, unit: feedUnit(c.mode)});
    return {line: e.line, section: e.section, phase: e.phase, type: c.type, provenance: e.provenance, instruction: lines[e.line - 1], quantities,
      invalidated: Object.entries(e.stateChange).filter(([, change]) => change.after === null || (Array.isArray(change.after) && change.after.every(v => v === null))).map(([key]) => key)};
  });
  return {schema: "linuxcnc-next-nc/process-summary/1", gcodeSHA256, units: program.model.units,
    coordinates: program.model.machine === "mill" ? "XYZ Cartesian" : "XZ, X radius", operations, policyCommands};
}
module.exports = {processSummary};
