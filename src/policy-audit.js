"use strict";
const {invariant} = require("./internal-error");

// Run after completeness has verified the boundary sequence and all source/plan
// uses. Fold COMMANDS, never emitter modalState/stateChange/request caches. The
// obligations below come from the decoded job and reviewed transition plan.
function auditPolicy(program, plan, commands, boundaries) {
  const units = program.model.units, defaultPlane = program.model.machine === "mill" ? "XY" : "XZ";
  let state = {}, sectionIndex = -1, section, transition, path, waypoint = 0, changedTool = false;
  let needsTool = false, active = false, finished = false, endWaypoints = 0, boundaryIndex = 0;
  let lastDirection = null, stoppedSinceStart = false, line = 1, boundary = "startup";
  const checked = {startup: 0, retractWaypoints: 0, toolChanges: 0, approachWaypoints: 0,
    operationEntries: 0, pathUses: 0, continuations: 0, linkWaypoints: 0, reversals: 0, endWaypoints: 0, programEnd: 0};
  const stopped = {spindleDirection: "stopped", coolant: "off", spindleMode: "rpm", spindleSpeed: 0, spindleMaximumRPM: null};
  const initialized = {units, distanceMode: "absolute", arcDistanceMode: "incremental", diameterMode: false,
    cutterCompensation: "off", cycle: "off", motionControl: "exactPath", temporaryOffsets: "cleared"};
  const mapped = () => plan.tools[`${section.tool.number}:${section.tool.offset}`];
  const mode = () => transition?.mode || "retract";
  const phase = () => !active ? (finished ? "program-end" : "header") : path ? "toolpath" : mode() === "link" ? "link" : "transition";
  function check(ok, rule, expected, pointer, observed = state) {
    if (ok) return;
    invariant(false, "POLICY_" + rule, "Required translator policy was not established at " + boundary + ".", {
      audit: "policy", line, boundary, phase: phase(),
      ...(active ? {section: sectionIndex + 1, operation: section.name, ...(path ? {path} : {})} : {}),
      ...(pointer ? {planPointer: pointer} : {}), expected, observed: {...observed}
    });
  }
  function requireState(expected, rule, pointer) {
    // Retain explicit unknown fields in failure diagnostics after an M6.
    if (!Object.entries(expected).every(([k,v]) => state[k] === v)) {
      check(false, rule, expected, pointer, {...state, ...Object.fromEntries(Object.keys(expected).map(k => [k, state[k] ?? null]))});
    }
  }
  function mapping() {
    return {...initialized, tool: mapped().tool, toolOffset: mapped().offset, workOffset: plan.workOffsets[section.workOffset]};
  }
  function process(spindle, coolant, feed, rule) {
    requireState({...mapping(), spindleMode: spindle.mode,
      spindleSpeed: spindle.mode === "css" ? spindle.speed / (units === "mm" ? 1000 : 12) : spindle.speed,
      spindleMaximumRPM: spindle.mode === "css" ? spindle.maximumRPM : null,
      spindleDirection: spindle.clockwise ? "clockwise" : "counterclockwise", coolant,
      ...(feed ? {feedMode: feed.mode, feedRate: feed.value} : {})}, rule);
  }
  function onBoundary(b) {
    boundary = b.type;
    if (b.type === "section-start") {
      if (sectionIndex < 0) {
        requireState({...initialized, ...stopped, plane: defaultPlane}, "STARTUP"); ++checked.startup;
      }
      sectionIndex = b.section - 1; section = program.model.sections[sectionIndex];
      transition = plan.sections[sectionIndex]; active = true; path = undefined; waypoint = 0; changedTool = false;
      needsTool = mapped().tool !== state.tool;
      if (mode() !== "retract") {
        process(section.initialSpindle, section.initialCoolant, null, "CONTINUITY");
        if (mode() === "continue") ++checked.continuations;
      }
    } else if (b.type === "path-start") {
      if (b.path === 1) {
        const required = mode() === "retract" ? transition.retract.length + transition.approach.length : mode() === "link" ? transition.moves.length : 0;
        check(waypoint === required && (mode() !== "retract" || changedTool === needsTool), "ENTRY_ORDER",
          {waypoints: required, toolChangeRequired: mode() === "retract" && needsTool});
        process(section.initialSpindle, section.initialCoolant, null, "OPERATION_ENTRY"); ++checked.operationEntries;
      }
      path = b.path;
    } else if (b.type === "path-end") path = undefined;
    else if (b.type === "section-end") { active = false; finished = true; }
  }
  for (let i = 0; i <= commands.length; i++) {
    line = i + 1;
    while (boundaryIndex < boundaries.length && boundaries[boundaryIndex].at === i) onBoundary(boundaries[boundaryIndex++]);
    if (i === commands.length) break;
    const c = commands[i].command;
    boundary = active ? path ? "path-use" : "operation-transition" : finished ? "program-end" : "startup";
    const isUse = ["rapid", "linear", "arc", "dwell"].includes(c.type);
    if (isUse) {
      if (active && path) {
        const p = section.paths[path - 1];
        process(p.spindle, p.coolant, p.feed, "PATH_STATE");
        if (p.kind === "arc") requireState({plane: p.plane || "XZ"}, "PATH_PLANE");
        ++checked.pathUses;
      } else if (active && mode() === "retract") {
        const retracting = waypoint < transition.retract.length;
        const pointer = `/sections/${sectionIndex}/${retracting ? "retract" : "approach"}/${retracting ? waypoint : waypoint - transition.retract.length}`;
        boundary = retracting ? "retract-waypoint" : "approach-waypoint";
        requireState({...initialized, ...stopped, ...(!retracting ? {...mapping(), plane: defaultPlane} : {})}, retracting ? "RETRACT_STATE" : "APPROACH_STATE", pointer);
        check(retracting ? !changedTool : changedTool === needsTool, "TOOL_CHANGE_ORDER", {toolChangeRequired: needsTool, retracting}, pointer);
        ++checked[retracting ? "retractWaypoints" : "approachWaypoints"]; ++waypoint;
      } else if (active) {
        process(section.initialSpindle, section.initialCoolant, null, "CONTINUITY"); ++checked.linkWaypoints; ++waypoint;
      } else {
        boundary = "end-retract-waypoint";
        requireState({...initialized, ...stopped}, "END_RETRACT_STATE", `/end/${endWaypoints}`);
        ++endWaypoints; ++checked.endWaypoints;
      }
    }
    // Mapping changes are confined to the reviewed retract/tool-change boundary.
    if (["toolChange", "workOffset", "toolOffset"].includes(c.type)) {
      check(active && !path && mode() === "retract" && waypoint === transition.retract.length,
        "MAPPING_ORDER", {afterRetract: true, beforeApproach: true});
      const tool = mapped(), pointer = `/tools/${section.tool.number}:${section.tool.offset}`;
      if (c.type === "toolChange") {
        requireState({...initialized, ...stopped, toolOffset: 0}, "TOOL_CHANGE_PREPARATION", pointer);
        check(needsTool && !changedTool && c.tool === tool.tool, "TOOL_CHANGE", {tool: tool.tool, toolChangeRequired: needsTool}, pointer);
        changedTool = true; ++checked.toolChanges;
      } else {
        requireState({...initialized, ...stopped, plane: defaultPlane, tool: tool.tool}, "MAPPING_STATE", pointer);
        if (c.type === "workOffset") check(c.value === plan.workOffsets[section.workOffset], "WORK_OFFSET", {workOffset: plan.workOffsets[section.workOffset]}, `/workOffsets/${section.workOffset}`);
        else {
          requireState({workOffset: plan.workOffsets[section.workOffset]}, "OFFSET_ORDER", pointer);
          check(c.offset === tool.offset, "TOOL_OFFSET", {toolOffset: tool.offset}, pointer);
        }
      }
    }
    if (["initialize", "clearTemporaryOffsets", "cancelToolOffset"].includes(c.type)) {
      const setup = !active && !finished || active && !path && mode() === "retract" && waypoint === transition.retract.length;
      check(setup || c.type === "cancelToolOffset" && !active && finished && endWaypoints === plan.end.length,
        "RESET_ORDER", {setupBeforeApproach: true, finalOffsetCancellationAfterRetract: true});
    }
    switch (c.type) {
      case "initialize": Object.assign(state, {units: c.units, plane: c.plane, distanceMode: "absolute", arcDistanceMode: "incremental", diameterMode: false, cutterCompensation: "off", cycle: "off", motionControl: "exactPath", feedMode: "perMinute"}); break;
      case "clearTemporaryOffsets": state.temporaryOffsets = "cleared"; break;
      case "spindleStop": state.spindleDirection = "stopped"; stoppedSinceStart = true; break;
      case "spindleMode": Object.assign(state, {spindleMode: c.mode, spindleSpeed: c.speed, spindleMaximumRPM: c.mode === "css" ? c.maximumRPM : null}); break;
      case "spindleStart": {
        const direction = c.clockwise ? "clockwise" : "counterclockwise";
        if (lastDirection !== null && lastDirection !== direction) {
          check(stoppedSinceStart, "REVERSAL_STOP", {stopBeforeReversal: true}); ++checked.reversals;
        }
        check(state.spindleDirection === "stopped" || state.spindleDirection === direction, "SPINDLE_START", {stoppedOrAlreadyRunning: direction});
        state.spindleDirection = direction; lastDirection = direction; stoppedSinceStart = false; break;
      }
      case "coolant": state.coolant = c.value === "off" ? "off" : state.coolant === "off" || state.coolant === c.value ? c.value : "unknown-or-combined"; break;
      case "feed":
        if (!active && finished) check(endWaypoints === plan.end.length && state.toolOffset === 0,
          "END_FEED_ORDER", {afterEndRetract: true, toolOffset: 0});
        state.feedMode = c.mode; if (c.value !== undefined) state.feedRate = c.value; break;
      case "cancelToolOffset": state.toolOffset = 0; break;
      case "toolChange": state = {tool: c.tool}; lastDirection = null; stoppedSinceStart = false; break;
      case "workOffset": state.workOffset = c.value; break;
      case "toolOffset": state.toolOffset = c.offset; break;
      case "plane": state.plane = c.value; break;
      case "end":
        check(!active && finished && endWaypoints === plan.end.length && i === commands.length - 1, "END_ORDER", {endWaypoints: plan.end.length, terminal: true});
        requireState({...stopped, toolOffset: 0, feedMode: "perMinute"}, "END_STATE"); ++checked.programEnd; break;
    }
    // A checked link/continue must retain compatible state throughout its
    // transition, not merely recover it at the first cutting move.
    if (active && !path && mode() !== "retract") process(section.initialSpindle, section.initialCoolant, null, "CONTINUITY");
  }
  check(checked.programEnd === 1 && checked.operationEntries === program.model.sections.length, "COMPLETION", {programEnd: 1, operationEntries: program.model.sections.length});
  return {schema: "linuxcnc-next-nc/policy-audit/1", status: "passed", checked,
    scope: "Independent command-state obligations at startup, retracts, tool changes, operation/path entry, links, reversals and shutdown. Not physical feedback, remap validation or machine acceptance."};
}
module.exports = {auditPolicy};
