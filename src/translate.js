"use strict";
const {readProgram} = require("./profile");
const {validatePlan, machineOf, axesOf} = require("./plan");
const {continuation, connection, exitPoint} = require("./continuity");
const {NextNCError} = require("./errors");
const {checkToolTable} = require("./tool-table");
const crypto = require("node:crypto");
const version = require("../package.json").version;
const {LinuxCNCOutput, decimal, comment} = require("./linuxcnc-output");
function translate(text, plan, options = {}) {
  const program = readProgram(text); validatePlan(plan, program);
  const toolTable = checkToolTable(options.toolTable, program, plan);
  const output = new LinuxCNCOutput(), {lines, sourceMap, requested} = output;
  const model = program.model, transitions = [], operationRanges = []; let context = {phase: "header"}, unchangedAxisWordsOmitted = 0;
  const machine = machineOf(program), axisIndices = axesOf(program).map(axis => "xyz".indexOf(axis));
  const planes = {XY: {indices: [0, 1]}, XZ: {indices: [0, 2]}, YZ: {indices: [1, 2]}};
  const defaultPlane = machine === "mill" ? "XY" : "XZ";
  function emit(command, detail = {}) { output.emit(command, context, detail); }
  const common = {type: "initialize", units: model.units, plane: defaultPlane};
  emit({type: "comment", text: `LinuxCNCNext-NC ${version}; experimental; reviewed execution plan required`});
  emit({type: "comment", text: `Program fingerprint ${program.report.programFingerprint.value}`});
  // Prefix user text so it cannot become an active LinuxCNC comment such as
  // (ABORT,...), (LOGOPEN,...) or (DEBUG,...).
  emit({type: "comment", text: "Program: " + model.name}); emit(common); emit({type: "clearTemporaryOffsets"}); emit({type: "spindleStop"}); emit({type: "coolant", value: "off"}); emit({type: "spindleMode", mode: "rpm", speed: 0});
  function stop() { emit({type: "spindleStop"}); emit({type: "coolant", value: "off"}); emit({type: "spindleMode", mode: "rpm", speed: 0}); requested.spindle = null; requested.coolant = "off"; requested.feed = null; }
  function waypoints(points, machine, stage) { points.forEach((p, index) => { const axis = Object.keys(p)[0]; emit({type: "rapid", frame: machine ? "machine" : "work", axes: {[axis]: p[axis]}}, {action: "motion", stage, waypoint: index + 1, motion: {kind: "rapid", frame: machine ? "machine" : "work", target: {...p}}}); }); }
  function state(s, c, f) {
    const key = JSON.stringify(s);
    if (requested.spindle !== key) {
      if (requested.spindle !== null && JSON.parse(requested.spindle).clockwise !== s.clockwise) emit({type: "spindleStop"});
      emit({type: "spindleMode", mode: s.mode, speed: s.mode === "css" ? s.speed / (model.units === "mm" ? 1000 : 12) : s.speed, ...(s.mode === "css" ? {maximumRPM: s.maximumRPM} : {})});
      emit({type: "spindleStart", clockwise: s.clockwise}); requested.spindle = key;
    }
    if (requested.coolant !== c) { emit({type: "coolant", value: "off"}); if (c === "flood") emit({type: "coolant", value: "flood"}); if (c === "mist") emit({type: "coolant", value: "mist"}); requested.coolant = c; }
    if (f && JSON.stringify(f) !== requested.feed) { emit({type: "feed", mode: f.mode, value: f.value}); requested.feed = JSON.stringify(f); }
  }
  for (const [index, section] of model.sections.entries()) {
    const firstLine = lines.length + 1;
    context = {section: index + 1, operation: section.name, phase: "transition"};
    emit({type: "comment", text: `Section ${index + 1}: ${section.name}`});
    const transition = plan.sections[index], mode = transition.mode || "retract";
    const record = {section: index + 1, operation: section.name, mode,
      reason: mode === "continue" ? continuation(program, index, plan).reason : mode === "link" ? connection(program, index, plan).reason : "Execution plan specifies a machine retract and approach."};
    if (mode === "link") Object.assign(record, {start: exitPoint(model.sections[index - 1]), end: section.start, moves: transition.moves});
    transitions.push(record);
    if (mode === "retract") {
      // Header already established stopped state before the first approach.
      if (index > 0) stop();
      waypoints(plan.sections[index].retract, true, "retract");
      const tool = plan.tools[`${section.tool.number}:${section.tool.offset}`];
      if (tool.tool !== requested.selectedTool) { emit({type: "cancelToolOffset"}); emit({type: "toolChange", tool: tool.tool}); requested.selectedTool = tool.tool; }
      // A site M6 remap may change modes. Reassert the translation contract.
      emit(common); emit({type: "clearTemporaryOffsets"}); emit({type: "workOffset", value: plan.workOffsets[section.workOffset]}); emit({type: "toolOffset", offset: tool.offset});
      waypoints(plan.sections[index].approach, false, "approach");
    } else if (mode === "link") {
      const boundaryContext = context;
      context = {...boundaryContext, phase: "link"};
      waypoints(transition.moves, false, "link");
      context = boundaryContext;
    }
    state(section.initialSpindle, section.initialCoolant, null);
    let position = section.start;
    for (const [p, path] of section.paths.entries()) {
      context = {section: index + 1, operation: section.name, phase: "toolpath", path: p + 1, kind: path.kind};
      try {
        state(path.spindle, path.coolant, path.feed);
        if (path.kind === "dwell") emit({type: "dwell", frame: "work", seconds: path.seconds}, {action: "dwell", seconds: path.seconds, position: [...position], frame: "work"});
        else if (path.kind === "arc") {
          const plane = path.plane || "XZ", frame = planes[plane];
          if (output.state.plane !== plane) emit({type: "plane", value: plane});
          const axes = {}, centerOffset = {};
          if (!path.fullCircle) for (const axis of frame.indices) axes["xyz"[axis]] = path.end[axis];
          for (const axis of frame.indices) centerOffset["xyz"[axis]] = path.center[axis] - path.start[axis];
          emit({type: "arc", frame: "work", plane, axes, centerOffset, clockwise: path.clockwise, fullCircle: path.fullCircle}, {action: "motion", motion: {kind: "arc", frame: "work", ...(machine === "mill" ? {plane} : {}), start: [...position], end: [...(path.fullCircle ? position : path.end)], centerOffset: path.center.map((value, axis) => frame.indices.includes(axis) ? value - path.start[axis] : 0), sourceStart: [...path.start], sourceCenter: [...path.center], clockwise: path.clockwise, fullCircle: path.fullCircle}});
          if (!path.fullCircle) position = path.end;
        } else for (let vertex = 1; vertex < path.points.length; vertex++) {
          const start = position, end = path.points[vertex];
          let axes = {};
          for (const axis of axisIndices) if (end[axis] !== start[axis]) axes["xyz"[axis]] = end[axis];
          // Retain a zero-length motion block if present; no vertices disappear.
          if (!Object.keys(axes).length) axes = Object.fromEntries(axisIndices.map(axis => ["xyz"[axis], end[axis]]));
          else unchangedAxisWordsOmitted += axisIndices.filter(axis => end[axis] === start[axis]).length;
          emit({type: path.kind, frame: "work", axes}, {action: "motion", segment: vertex, fromVertex: vertex, toVertex: vertex + 1, motion: {kind: path.kind, frame: "work", start: [...start], end: [...end]}});
          position = end;
        }
      } catch (error) { if (error instanceof NextNCError) error.context = {...context, ...error.context}; throw error; }
    }
    operationRanges.push({section: index + 1, operation: section.name, firstLine, lastLine: lines.length, tool: section.tool, mappedTool: plan.tools[`${section.tool.number}:${section.tool.offset}`], workOffset: section.workOffset, mappedWorkOffset: plan.workOffsets[section.workOffset], initialSpindle: section.initialSpindle, initialCoolant: section.initialCoolant, toolpathMotions: sourceMap.slice(firstLine - 1).filter(item => item.phase === "toolpath" && item.action === "motion").length});
  }
  context = {phase: "program-end"}; stop(); waypoints(plan.end, true, "program-end"); emit({type: "cancelToolOffset"}); emit({type: "feed", mode: "perMinute"}); emit({type: "end"});
  const gcode = lines.join("\n") + "\n";
  return {gcode, report: {...program.report, translator: version, gcodeLines: lines.length, toolTable,
    traceability: {schema: "linuxcnc-next-nc/source-map/1", lineNumbers: "one-based", vertexNumbers: "one-based within each decoded path", coordinates: machine === "mill" ? "XYZ Cartesian, program units" : "XYZ, program units, X radius", gcodeSHA256: crypto.createHash("sha256").update(gcode).digest("hex"), operationRanges},
    execution: {planSchema: plan.schema, transitions, continuations: transitions.filter(t => t.mode === "continue").length,
      links: transitions.filter(t => t.mode === "link").length, unchangedAxisWordsOmitted, coordinatesRounded: false}}, sourceMap};
}
module.exports = {translate, decimal, comment};
