"use strict";
const {readProgram} = require("./profile");
const {validatePlan} = require("./plan");
const {continuation, connection, exitPoint} = require("./continuity");
const {requireValue: need, NextNCError} = require("./errors");
const {checkToolTable} = require("./tool-table");
const crypto = require("node:crypto");
const version = require("../package.json").version;
function decimal(n) {
  need(Number.isFinite(n) && Math.abs(n) < 1e15, "NUMBER", "Cannot encode a nonfinite or excessive G-code value.");
  if (n === 0) return "0";
  const sign = n < 0 ? "-" : "", value = String(Math.abs(n));
  if (!/[eE]/.test(value)) return sign + value;
  const [mantissa, exponent] = value.toLowerCase().split("e"), parts = mantissa.split(".");
  const digits = parts.join(""), position = parts[0].length + Number(exponent);
  need(Math.abs(position) < 100, "PRECISION", "Value requires excessive decimal precision for LinuxCNC.");
  return sign + (position <= 0 ? "0." + "0".repeat(-position) + digits : position >= digits.length ? digits + "0".repeat(position - digits.length) : digits.slice(0, position) + "." + digits.slice(position));
}
function comment(text) { return "(" + String(text).replace(/[^\x20-\x7E]|[();%]/g, "_").slice(0, 150) + ")"; }
function translate(text, plan, options = {}) {
  const program = readProgram(text); validatePlan(plan, program);
  const toolTable = checkToolTable(options.toolTable, program, plan);
  const model = program.model, lines = [], sourceMap = [], transitions = [], operationRanges = []; let context = {phase: "header"}, unchangedAxisWordsOmitted = 0;
  function emit(line, detail = {}) { need(line.length <= 240, "LINE_LENGTH", "Generated line exceeds 240 characters.", context); lines.push(line); sourceMap.push({line: lines.length, action: "state", ...context, ...detail}); }
  const common = (model.units === "mm" ? "G21" : "G20") + " G18 G8 G90 G91.1 G40 G80 G94 G61";
  emit(comment(`LinuxCNCNext-NC ${version}; experimental; reviewed execution plan required`));
  emit(comment(`Program fingerprint ${program.report.programFingerprint.value}`));
  // Prefix user text so it cannot become an active LinuxCNC comment such as
  // (ABORT,...), (LOGOPEN,...) or (DEBUG,...).
  emit(comment("Program: " + model.name)); emit(common); emit("G92.1"); emit("M5 $0"); emit("M9"); emit("G97 S0 $0");
  let spindle = null, coolant = "off", feed = null, selectedTool = null;
  function stop() { emit("M5 $0"); emit("M9"); emit("G97 S0 $0"); spindle = null; coolant = "off"; feed = null; }
  function waypoints(points, machine, stage) { points.forEach((p, index) => { const axis = Object.keys(p)[0]; emit((machine ? "G53 " : "") + "G0 " + axis.toUpperCase() + decimal(p[axis]), {action: "motion", stage, waypoint: index + 1, motion: {kind: "rapid", frame: machine ? "machine" : "work", target: {...p}}}); }); }
  function state(s, c, f) {
    const key = JSON.stringify(s);
    if (spindle !== key) {
      if (spindle !== null && JSON.parse(spindle).clockwise !== s.clockwise) emit("M5 $0");
      emit(s.mode === "css" ? `G96 D${decimal(s.maximumRPM)} S${decimal(s.speed / (model.units === "mm" ? 1000 : 12))} $0` : `G97 S${decimal(s.speed)} $0`);
      emit(s.clockwise ? "M3 $0" : "M4 $0"); spindle = key;
    }
    if (coolant !== c) { emit("M9"); if (c === "flood") emit("M8"); if (c === "mist") emit("M7"); coolant = c; }
    if (f && JSON.stringify(f) !== feed) { emit((f.mode === "perRevolution" ? "G95" : "G94") + " F" + decimal(f.value)); feed = JSON.stringify(f); }
  }
  for (const [index, section] of model.sections.entries()) {
    const firstLine = lines.length + 1;
    context = {section: index + 1, operation: section.name, phase: "transition"};
    emit(comment(`Section ${index + 1}: ${section.name}`));
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
      if (tool.tool !== selectedTool) { emit("G49"); emit(`T${tool.tool} M6`); selectedTool = tool.tool; }
      // A site M6 remap may change modes. Reassert the translation contract.
      emit(common); emit("G92.1"); emit(plan.workOffsets[section.workOffset]); emit(`G43 H${tool.offset}`);
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
        if (path.kind === "dwell") emit("G4 P" + decimal(path.seconds), {action: "dwell", seconds: path.seconds, position: [...position], frame: "work"});
        else if (path.kind === "arc") {
          let block = path.clockwise ? "G2" : "G3";
          if (!path.fullCircle) block += ` X${decimal(path.end[0])} Z${decimal(path.end[2])}`;
          block += ` I${decimal(path.center[0] - path.start[0])} K${decimal(path.center[2] - path.start[2])}`;
          if (path.fullCircle) block += " P1";
          emit(block, {action: "motion", motion: {kind: "arc", frame: "work", start: [...position], end: [...(path.fullCircle ? position : path.end)], centerOffset: [path.center[0] - path.start[0], 0, path.center[2] - path.start[2]], sourceStart: [...path.start], sourceCenter: [...path.center], clockwise: path.clockwise, fullCircle: path.fullCircle}});
          if (!path.fullCircle) position = path.end;
        } else for (let vertex = 1; vertex < path.points.length; vertex++) {
          const start = position, end = path.points[vertex];
          let axes = "";
          if (end[0] !== start[0]) axes += ` X${decimal(end[0])}`;
          if (end[2] !== start[2]) axes += ` Z${decimal(end[2])}`;
          // Retain a zero-length motion block if present; no vertices disappear.
          if (!axes) axes = ` X${decimal(end[0])} Z${decimal(end[2])}`;
          else unchangedAxisWordsOmitted += Number(end[0] === start[0]) + Number(end[2] === start[2]);
          emit((path.kind === "rapid" ? "G0" : "G1") + axes, {action: "motion", segment: vertex, fromVertex: vertex, toVertex: vertex + 1, motion: {kind: path.kind, frame: "work", start: [...start], end: [...end]}});
          position = end;
        }
      } catch (error) { if (error instanceof NextNCError) error.context = {...context, ...error.context}; throw error; }
    }
    operationRanges.push({section: index + 1, operation: section.name, firstLine, lastLine: lines.length, toolpathMotions: sourceMap.slice(firstLine - 1).filter(item => item.phase === "toolpath" && item.action === "motion").length});
  }
  context = {phase: "program-end"}; stop(); waypoints(plan.end, true, "program-end"); emit("G49"); emit("G94"); emit("M2");
  const gcode = lines.join("\n") + "\n";
  return {gcode, report: {...program.report, translator: version, gcodeLines: lines.length, toolTable,
    traceability: {schema: "linuxcnc-next-nc/source-map/1", lineNumbers: "one-based", vertexNumbers: "one-based within each decoded path", coordinates: "XYZ, program units, X radius", gcodeSHA256: crypto.createHash("sha256").update(gcode).digest("hex"), operationRanges},
    execution: {planSchema: plan.schema, transitions, continuations: transitions.filter(t => t.mode === "continue").length,
      links: transitions.filter(t => t.mode === "link").length, unchangedAxisWordsOmitted, coordinatesRounded: false}}, sourceMap};
}
module.exports = {translate, decimal, comment};
