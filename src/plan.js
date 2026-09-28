"use strict";
const {requireValue: need} = require("./errors");
const {continuation, connection, exitPoint} = require("./continuity");
const schema = "linuxcnc-next-nc/execution-plan/3";
const continuationSchema = "linuxcnc-next-nc/execution-plan/2";
const legacySchema = "linuxcnc-next-nc/execution-plan/1";
const coordinates = ["G54", "G55", "G56", "G57", "G58", "G59", "G59.1", "G59.2", "G59.3"];
function keys(value, allowed, label) {
  need(value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).every(k => allowed.includes(k)), "PLAN", `Invalid or unknown fields in ${label}.`);
}
function moves(value, label, start) {
  need(Array.isArray(value) && value.length >= (start ? 1 : 2) && value.length <= 1000, "TRANSITION", `${label} requires explicit ordered X/Z waypoints; missing paths are not inferred.`);
  const end = start ? {x: start[0], z: start[2]} : {};
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
  need([schema, continuationSchema, legacySchema].includes(plan.schema), "PLAN_SCHEMA", "Unsupported execution plan schema.");
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
    const transition = plan.sections[i];
    if (plan.schema !== legacySchema) {
      const linkSupported = plan.schema === schema;
      keys(transition, linkSupported ? ["mode", "retract", "approach", "moves"] : ["mode", "retract", "approach"], `section ${i + 1}`);
      need((linkSupported ? ["continue", "retract", "link"] : ["continue", "retract"]).includes(transition.mode), "TRANSITION", `Section mode must be ${linkSupported ? "continue, retract or link" : "continue or retract"}.`, context);
      if (transition.mode === "continue") {
        keys(transition, ["mode"], `section ${i + 1} continuation`);
        const check = continuation(program, i, plan);
        need(check.eligible, "CONTINUATION", check.reason, context);
        return;
      }
      if (transition.mode === "link") {
        keys(transition, ["mode", "moves"], `section ${i + 1} link`);
        const check = connection(program, i, plan);
        need(check.eligible, "LINK_STATE", check.reason, context);
        const start = exitPoint(program.model.sections[i - 1]);
        const end = moves(transition.moves, `Section ${i + 1} work-coordinate link`, start);
        need(end.x === s.start[0] && end.z === s.start[2], "LINK_ENDPOINT", "The reviewed link must end exactly at the next operation's entry X/Z.", {...context, start, expected: s.start, actual: [end.x, 0, end.z]});
        return;
      }
      keys(transition, ["mode", "retract", "approach"], `section ${i + 1} retract`);
    } else keys(transition, ["retract", "approach"], `section ${i + 1}`);
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
    tools, workOffsets, sections: program.model.sections.map((s, i) => continuation(program, i).eligible ? {mode: "continue"} : {mode: "retract", retract: null, approach: null}), end: null};
}
module.exports = {validatePlan, template, schema, legacySchema};
