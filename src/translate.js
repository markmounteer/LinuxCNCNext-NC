"use strict";
const {readProgram} = require("./profile");
const {validatePlan} = require("./plan");
const {continuation} = require("./continuity");
const {requireValue: need, NextNCError} = require("./errors");
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
function translate(text, plan) {
  const program = readProgram(text); validatePlan(plan, program);
  const model = program.model, lines = [], sourceMap = [], transitions = []; let context = {}, unchangedAxisWordsOmitted = 0;
  function emit(line) { need(line.length <= 240, "LINE_LENGTH", "Generated line exceeds 240 characters.", context); lines.push(line); sourceMap.push({...context}); }
  const common = (model.units === "mm" ? "G21" : "G20") + " G18 G8 G90 G91.1 G40 G80 G94 G61";
  emit(comment(`LinuxCNCNext-NC ${version}; experimental; reviewed execution plan required`));
  emit(comment(`Program fingerprint ${program.report.programFingerprint.value}`));
  // Prefix user text so it cannot become an active LinuxCNC comment such as
  // (ABORT,...), (LOGOPEN,...) or (DEBUG,...).
  emit(comment("Program: " + model.name)); emit(common); emit("G92.1"); emit("M5 $0"); emit("M9"); emit("G97 S0 $0");
  let spindle = null, coolant = "off", feed = null, selectedTool = null;
  function stop() { emit("M5 $0"); emit("M9"); emit("G97 S0 $0"); spindle = null; coolant = "off"; feed = null; }
  function waypoints(points, machine) { for (const p of points) { const axis = Object.keys(p)[0]; emit((machine ? "G53 " : "") + "G0 " + axis.toUpperCase() + decimal(p[axis])); } }
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
    context = {section: index + 1, operation: section.name, phase: "transition"};
    emit(comment(`Section ${index + 1}: ${section.name}`));
    const continuous = plan.sections[index].mode === "continue";
    transitions.push({section: index + 1, operation: section.name, mode: continuous ? "continue" : "retract",
      reason: continuous ? continuation(program, index, plan).reason : "Execution plan specifies a machine retract and approach."});
    if (!continuous) {
      // Header already established stopped state before the first approach.
      if (index > 0) stop();
      waypoints(plan.sections[index].retract, true);
      const tool = plan.tools[`${section.tool.number}:${section.tool.offset}`];
      if (tool.tool !== selectedTool) { emit("G49"); emit(`T${tool.tool} M6`); selectedTool = tool.tool; }
      // A site M6 remap may change modes. Reassert the translation contract.
      emit(common); emit("G92.1"); emit(plan.workOffsets[section.workOffset]); emit(`G43 H${tool.offset}`);
      waypoints(plan.sections[index].approach, false);
    }
    state(section.initialSpindle, section.initialCoolant, null);
    let position = section.start;
    for (const [p, path] of section.paths.entries()) {
      context = {section: index + 1, operation: section.name, path: p + 1, kind: path.kind};
      try {
        state(path.spindle, path.coolant, path.feed);
        if (path.kind === "dwell") emit("G4 P" + decimal(path.seconds));
        else if (path.kind === "arc") {
          let block = path.clockwise ? "G2" : "G3";
          if (!path.fullCircle) block += ` X${decimal(path.end[0])} Z${decimal(path.end[2])}`;
          block += ` I${decimal(path.center[0] - path.start[0])} K${decimal(path.center[2] - path.start[2])}`;
          if (path.fullCircle) block += " P1";
          emit(block);
          if (!path.fullCircle) position = path.end;
        } else for (let vertex = 1; vertex < path.points.length; vertex++) {
          const start = position, end = path.points[vertex];
          let axes = "";
          if (end[0] !== start[0]) axes += ` X${decimal(end[0])}`;
          if (end[2] !== start[2]) axes += ` Z${decimal(end[2])}`;
          // Retain a zero-length motion block if present; no vertices disappear.
          if (!axes) axes = ` X${decimal(end[0])} Z${decimal(end[2])}`;
          else unchangedAxisWordsOmitted += Number(end[0] === start[0]) + Number(end[2] === start[2]);
          emit((path.kind === "rapid" ? "G0" : "G1") + axes);
          position = end;
        }
      } catch (error) { if (error instanceof NextNCError) error.context = {...context, ...error.context}; throw error; }
    }
  }
  context = {phase: "program-end"}; stop(); waypoints(plan.end, true); emit("G49"); emit("G94"); emit("M2");
  return {gcode: lines.join("\n") + "\n", report: {...program.report, translator: version, gcodeLines: lines.length,
    execution: {planSchema: plan.schema, transitions, continuations: transitions.filter(t => t.mode === "continue").length, unchangedAxisWordsOmitted, coordinatesRounded: false}}, sourceMap};
}
module.exports = {translate, decimal, comment};
