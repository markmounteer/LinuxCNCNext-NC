"use strict";
const {requireValue: need} = require("./errors");
const schema = "linuxcnc-next-nc/execution-plan/1";
const coordinates = ["G54", "G55", "G56", "G57", "G58", "G59", "G59.1", "G59.2", "G59.3"];
function keys(value, allowed, label) {
  need(value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).every(k => allowed.includes(k)), "PLAN", `Invalid or unknown fields in ${label}.`);
}
function moves(value, label) {
  need(Array.isArray(value) && value.length >= 2 && value.length <= 1000, "TRANSITION", `${label} requires explicit ordered X/Z waypoints; missing paths are not inferred.`);
  const end = {};
  for (const move of value) {
    keys(move, ["x", "z"], label);
    need(Object.keys(move).length === 1, "TRANSITION", `${label}: each waypoint must move one axis explicitly.`);
    const axis = Object.keys(move)[0];
    need(Number.isFinite(move[axis]) && Math.abs(move[axis]) < 1e9, "TRANSITION", `${label}: ${axis} must be a finite coordinate below 1e9 in magnitude.`);
    end[axis] = move[axis];
  }
  need(Object.hasOwn(end, "x") && Object.hasOwn(end, "z"), "TRANSITION", `${label} must establish both X and Z.`);
  return end;
}
function validatePlan(plan, program) {
  keys(plan, ["schema", "programFingerprint", "units", "tools", "workOffsets", "sections", "end"], "execution plan");
  need(plan.schema === schema, "PLAN_SCHEMA", "Unsupported execution plan schema.");
  need(plan.programFingerprint === program.report.programFingerprint.value, "PLAN_MISMATCH", "The execution plan belongs to a different decoded program. Regenerate/review the plan; do not reuse unreviewed entry moves.");
  need(plan.units === program.model.units, "PLAN_UNITS", "Plan coordinates must use the same units as the program.");
  need(plan.tools && typeof plan.tools === "object" && !Array.isArray(plan.tools), "TOOL_MAPPING", "Explicit tool mappings are required.");
  need(plan.workOffsets && typeof plan.workOffsets === "object" && !Array.isArray(plan.workOffsets), "WCS_MAPPING", "Explicit work-offset mappings are required.");
  need(Array.isArray(plan.sections) && plan.sections.length === program.model.sections.length, "TRANSITION", "One transition plan is required for each operation.");
  program.model.sections.forEach((s, i) => {
    const context = {section: i + 1, operation: s.name, tool: s.tool.number};
    const key = `${s.tool.number}:${s.tool.offset}`, tool = plan.tools[key];
    need(Object.hasOwn(plan.tools, key) && tool, "TOOL_MAPPING", `Map Fusion tool:offset ${key} to LinuxCNC tool and offset records.`, context);
    keys(tool, ["tool", "offset"], `tool ${key}`);
    need([tool.tool, tool.offset].every(n => Number.isSafeInteger(n) && n > 0 && n <= 99999), "TOOL_MAPPING", "LinuxCNC tool and H-offset records must be explicit positive integers.", context);
    need(Object.hasOwn(plan.workOffsets, String(s.workOffset)) && coordinates.includes(plan.workOffsets[s.workOffset]), "WCS_MAPPING", `Map Fusion work offset ${s.workOffset} explicitly to G54..G59.3.`, context);
    const transition = plan.sections[i]; keys(transition, ["retract", "approach"], `section ${i + 1}`);
    moves(transition.retract, `Section ${i + 1} machine-coordinate retract`);
    const end = moves(transition.approach, `Section ${i + 1} work-coordinate approach`);
    need(end.x === s.start[0] && end.z === s.start[2], "ENTRY_MISMATCH", "The reviewed approach must end exactly at the operation's recorded entry X/Z.", {...context, expected: s.start, actual: [end.x, 0, end.z]});
  });
  moves(plan.end, "Program-end machine-coordinate retract");
  return plan;
}
function template(program) {
  const tools = {}, workOffsets = {};
  for (const s of program.model.sections) { tools[`${s.tool.number}:${s.tool.offset}`] = {tool: null, offset: null}; workOffsets[s.workOffset] = null; }
  return {schema, programFingerprint: program.report.programFingerprint.value, units: program.model.units,
    tools, workOffsets, sections: program.model.sections.map(() => ({retract: null, approach: null})), end: null};
}
module.exports = {validatePlan, template};
