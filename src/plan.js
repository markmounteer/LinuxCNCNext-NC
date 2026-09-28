"use strict";
const {requireValue: need, NextNCError} = require("./errors");
const {continuation, connection, exitPoint} = require("./continuity");
const schema = "linuxcnc-next-nc/execution-plan/3";
const machineSchema = "linuxcnc-next-nc/execution-plan/4";
const continuationSchema = "linuxcnc-next-nc/execution-plan/2";
const legacySchema = "linuxcnc-next-nc/execution-plan/1";
const coordinates = ["G54", "G55", "G56", "G57", "G58", "G59", "G59.1", "G59.2", "G59.3"];
const machineOf = program => program.model.machine === "mill" ? "mill" : "lathe";
const axesOf = program => machineOf(program) === "mill" ? ["x", "y", "z"] : ["x", "z"];
const coordinate = (point, axis) => point[{x: 0, y: 1, z: 2}[axis]];
function keys(value, allowed, label) {
  need(value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).every(k => allowed.includes(k)), "PLAN", `Invalid or unknown fields in ${label}.`);
}
function moves(value, label, axes, start) {
  need(Array.isArray(value) && value.length >= (start ? 1 : axes.length) && value.length <= 1000, "TRANSITION", `${label} requires explicit ordered ${axes.join("/").toUpperCase()} waypoints; missing paths are not inferred.`);
  const end = start ? Object.fromEntries(axes.map(axis => [axis, coordinate(start, axis)])) : {};
  for (const move of value) {
    keys(move, axes, label);
    need(Object.keys(move).length === 1, "TRANSITION", `${label}: each waypoint must move one axis explicitly.`);
    const axis = Object.keys(move)[0];
    need(Number.isFinite(move[axis]) && Math.abs(move[axis]) < 1e9, "TRANSITION", `${label}: ${axis} must be a finite coordinate below 1e9 in magnitude.`);
    end[axis] = move[axis];
  }
  need(axes.every(axis => Object.hasOwn(end, axis)), "TRANSITION", `${label} must establish ${axes.join("/").toUpperCase()}.`);
  return end;
}
function collectPlanIssues(plan, program) {
  const machine = machineOf(program), axes = axesOf(program);
  keys(plan, ["schema", "machine", "programFingerprint", "units", "tools", "workOffsets", "sections", "end"], "execution plan");
  need([schema, machineSchema, continuationSchema, legacySchema].includes(plan.schema), "PLAN_SCHEMA", "Unsupported execution plan schema.");
  need(plan.schema === machineSchema ? plan.machine === machine : machine === "lathe" && !Object.hasOwn(plan, "machine"), "PLAN_MACHINE", "XYZ milling requires execution-plan/4 with machine=mill; the plan machine must match the program.");
  need(plan.programFingerprint === program.report.programFingerprint.value, "PLAN_MISMATCH", "The execution plan belongs to a different decoded program. Regenerate/review the plan; do not reuse unreviewed entry moves.");
  need(plan.units === program.model.units, "PLAN_UNITS", "Plan coordinates must use the same units as the program.");
  need(plan.tools && typeof plan.tools === "object" && !Array.isArray(plan.tools), "TOOL_MAPPING", "Explicit tool mappings are required.");
  need(plan.workOffsets && typeof plan.workOffsets === "object" && !Array.isArray(plan.workOffsets), "WCS_MAPPING", "Explicit work-offset mappings are required.");
  need(Array.isArray(plan.sections) && plan.sections.length === program.model.sections.length, "TRANSITION", "One transition plan is required for each operation.");
  const issues = [], notChecked = [], mapped = [];
  function attempt(field, context, fn) {
    try { fn(); return true; }
    catch (error) {
      if (!(error instanceof NextNCError)) throw error;
      issues.push({code: error.code, message: error.message, context: {...context, ...error.context, field}, correction: "Edit this field in the execution plan, review the affected machine moves, and rerun preflight."});
      return false;
    }
  }
  program.model.sections.forEach((s, i) => {
    const context = {section: i + 1, operation: s.name, tool: s.tool.number};
    const key = `${s.tool.number}:${s.tool.offset}`, tool = plan.tools[key];
    const toolOK = attempt(`tools.${key}`, context, () => {
      need(Object.hasOwn(plan.tools, key) && tool, "TOOL_MAPPING", `Map Fusion tool:offset ${key} to LinuxCNC tool and offset records.`, context);
      keys(tool, ["tool", "offset"], `tool ${key}`);
      need([tool.tool, tool.offset].every(n => Number.isSafeInteger(n) && n > 0 && n <= 99999), "TOOL_MAPPING", "LinuxCNC tool and H-offset records must be explicit positive integers.", context);
    });
    const wcsOK = attempt(`workOffsets.${s.workOffset}`, context, () => {
      need(Object.hasOwn(plan.workOffsets, String(s.workOffset)) && coordinates.includes(plan.workOffsets[s.workOffset]), "WCS_MAPPING", `Map Fusion work offset ${s.workOffset} explicitly to G54..G59.3.`, context);
    });
    mapped[i] = toolOK && wcsOK;
  });
  program.model.sections.forEach((s, i) => {
    const context = {section: i + 1, operation: s.name, tool: s.tool.number};
    const transition = plan.sections[i], field = `sections[${i}]`;
    if (!attempt(field, context, () => {
      if (plan.schema === legacySchema) return keys(transition, ["retract", "approach"], `section ${i + 1}`);
      keys(transition, plan.schema === continuationSchema ? ["mode", "retract", "approach"] : ["mode", "retract", "approach", "moves"], `section ${i + 1}`);
      const modes = plan.schema === continuationSchema ? ["continue", "retract"] : ["continue", "retract", "link"];
      need(transition && modes.includes(transition.mode), "TRANSITION", `Section mode must be ${modes.join(", ")}.`, context);
      keys(transition, transition.mode === "continue" ? ["mode"] : transition.mode === "link" ? ["mode", "moves"] : ["mode", "retract", "approach"], `section ${i + 1}`);
    })) return;
    if (["continue", "link"].includes(transition.mode)) {
      if (i > 0 && (!mapped[i] || !mapped[i - 1])) {
        notChecked.push({...context, field, reason: "Boundary state checks require valid tool and work-offset mappings for both operations."});
        return;
      }
      attempt(field, context, () => {
        const check = transition.mode === "continue" ? continuation(program, i, plan) : connection(program, i, plan);
        need(check.eligible, transition.mode === "continue" ? "CONTINUATION" : "LINK_STATE", check.reason, context);
        if (transition.mode === "link") {
          const start = exitPoint(program.model.sections[i - 1]);
          const end = moves(transition.moves, `Section ${i + 1} work-coordinate link`, axes, start);
          need(axes.every(axis => end[axis] === coordinate(s.start, axis)), "LINK_ENDPOINT", `The reviewed link must end exactly at the next operation's entry ${axes.join("/").toUpperCase()}.`, {...context, start, expected: s.start, actual: [end.x, end.y || 0, end.z]});
        }
      });
      return;
    }
    attempt(`${field}.retract`, context, () => moves(transition.retract, `Section ${i + 1} machine-coordinate retract`, axes));
    attempt(`${field}.approach`, context, () => {
      const end = moves(transition.approach, `Section ${i + 1} work-coordinate approach`, axes);
      need(axes.every(axis => end[axis] === coordinate(s.start, axis)), "ENTRY_MISMATCH", `The reviewed approach must end exactly at the operation's recorded entry ${axes.join("/").toUpperCase()}.`, {...context, expected: s.start, actual: [end.x, end.y || 0, end.z]});
    });
  });
  attempt("end", {}, () => moves(plan.end, "Program-end machine-coordinate retract", axes));
  return {issues, notChecked};
}
function validatePlan(plan, program) {
  const {issues, notChecked} = collectPlanIssues(plan, program);
  if (issues.length) throw new NextNCError(issues[0].code, issues[0].message, {...issues[0].context, issues, notChecked});
  return plan;
}
function template(program) {
  const tools = {}, workOffsets = {}, machine = machineOf(program);
  for (const s of program.model.sections) { tools[`${s.tool.number}:${s.tool.offset}`] = {tool: null, offset: null}; workOffsets[s.workOffset] = null; }
  return {schema: machine === "mill" ? machineSchema : schema, ...(machine === "mill" ? {machine} : {}), programFingerprint: program.report.programFingerprint.value, units: program.model.units,
    tools, workOffsets, sections: program.model.sections.map((s, i) => continuation(program, i).eligible ? {mode: "continue"} : {mode: "retract", retract: null, approach: null}), end: null};
}
module.exports = {validatePlan, collectPlanIssues, template, schema, machineSchema, legacySchema, machineOf, axesOf};
