"use strict";
const {invariant} = require("./internal-error");
const fields = {
  comment: ["text"], initialize: ["units", "plane"], clearTemporaryOffsets: [], spindleStop: [],
  spindleMode: ["mode", "speed", "maximumRPM"], spindleStart: ["clockwise"], coolant: ["value"],
  feed: ["mode", "value"], cancelToolOffset: [], toolChange: ["tool"], workOffset: ["value"],
  toolOffset: ["offset"], plane: ["value"], rapid: ["frame", "axes"], linear: ["frame", "axes"],
  arc: ["frame", "plane", "axes", "centerOffset", "clockwise", "fullCircle"],
  dwell: ["frame", "seconds"], end: []
};
const planeAxes = {XY: ["x", "y"], XZ: ["x", "z"], YZ: ["y", "z"]};
const object = v => v !== null && typeof v === "object" && !Array.isArray(v) && Object.getPrototypeOf(v) === Object.prototype;
function validateCommand(c, output, context) {
  const check = (ok, rule, message) => invariant(ok, rule, message, context);
  const number = (v, minimum = -Infinity) => Number.isFinite(v) && Math.abs(v) < 1e15 && v >= minimum;
  const positive = v => number(v) && v > 0;
  const plane = v => Object.hasOwn(planeAxes, v) && (output.machine === "mill" || v === "XZ");
  check(object(c) && Object.hasOwn(fields, c.type), "COMMAND_TYPE", "Unknown internal machining command.");
  check(Object.keys(c).every(k => k === "type" || fields[c.type].includes(k)), "COMMAND_FIELDS", "Unexpected internal command field.");
  check(!output.state.ended, "COMMAND_AFTER_END", "Cannot emit a command after program end.");
  if (!["comment", "initialize"].includes(c.type)) check(output.started, "COMMAND_BEFORE_INIT", "Command requires program initialization.");
  switch (c.type) {
    case "comment": check(typeof c.text === "string", "COMMENT_TEXT", "Comment requires text."); break;
    case "initialize":
      check(["mm", "inch"].includes(c.units) && (!output.units || c.units === output.units), "COMMAND_UNITS", "Initialization must retain program units.");
      check(plane(c.plane), "COMMAND_PLANE", "Initialization plane is unsupported for this machine."); break;
    case "plane": check(plane(c.value), "COMMAND_PLANE", "Plane is unsupported for this machine."); break;
    case "spindleMode":
      check(["rpm", "css"].includes(c.mode) && (c.mode !== "css" || output.machine === "lathe"), "SPINDLE_MODE", "Spindle mode is unsupported for this machine.");
      check(number(c.speed, 0) && (c.mode === "css" ? positive(c.speed) && positive(c.maximumRPM) : c.maximumRPM === undefined), "SPINDLE_VALUE", "Invalid spindle speed or CSS cap."); break;
    case "spindleStart":
      check(typeof c.clockwise === "boolean", "SPINDLE_DIRECTION", "Spindle direction must be Boolean.");
      check(["rpm", "css"].includes(output.state.spindleMode) && positive(output.state.spindleSpeed), "SPINDLE_STATE", "Spindle start requires an established speed/mode."); break;
    case "coolant": check(["off", "flood", "mist"].includes(c.value), "COOLANT_VALUE", "Unsupported coolant command."); break;
    case "feed":
      check(["perMinute", "perRevolution"].includes(c.mode) && (c.value === undefined || positive(c.value)), "FEED_VALUE", "Invalid feed command."); break;
    case "toolChange": case "toolOffset": {
      const n = c.type === "toolChange" ? c.tool : c.offset;
      check(Number.isSafeInteger(n) && n > 0 && n <= 99999, "TOOL_ID", "Tool/H record must be a positive integer up to 99999."); break;
    }
    case "workOffset": check(["G54", "G55", "G56", "G57", "G58", "G59", "G59.1", "G59.2", "G59.3"].includes(c.value), "WORK_OFFSET", "Unsupported work-offset command."); break;
    case "dwell":
      check(c.frame === "work" && positive(c.seconds), "DWELL_VALUE", "Dwell requires a positive duration and work frame.");
      check(output.state.units === output.units && plane(output.state.plane), "MOTION_STATE", "Dwell requires established units and plane."); break;
    case "rapid": case "linear": case "arc": {
      const axes = output.machine === "mill" ? ["x", "y", "z"] : ["x", "z"];
      const validAxes = (values, allowed, empty = false) => object(values) && (empty || Object.keys(values).length > 0) && Object.entries(values).every(([axis, value]) => allowed.includes(axis) && number(value));
      check(["work", "machine"].includes(c.frame) && (c.frame !== "machine" || c.type === "rapid"), "MOTION_FRAME", "Unsupported motion coordinate frame.");
      check(validAxes(c.axes, axes, c.type === "arc" && c.fullCircle === true), "MOTION_AXES", "Motion has missing, unsupported or invalid axes.");
      check(output.state.units === output.units && output.state.distanceMode === "absolute" && plane(output.state.plane), "MOTION_STATE", "Motion requires established units, plane and absolute mode.");
      if (c.type === "arc") {
        check(plane(c.plane) && c.plane === output.state.plane, "ARC_PLANE", "Arc plane disagrees with the active plane.");
        check(typeof c.clockwise === "boolean" && typeof c.fullCircle === "boolean", "ARC_FLAGS", "Arc flags must be Boolean.");
        const active = planeAxes[c.plane];
        check(validAxes(c.centerOffset, active) && active.every(a => Object.hasOwn(c.centerOffset, a)), "ARC_CENTER", "Arc requires the two in-plane center offsets.");
        check(c.fullCircle ? Object.keys(c.axes).length === 0 : validAxes(c.axes, active) && active.every(a => Object.hasOwn(c.axes, a)), "ARC_ENDPOINT", "Arc endpoints must match the planar/full-circle contract.");
        check(output.state.arcDistanceMode === "incremental", "ARC_MODE", "Arc requires incremental center offsets.");
      }
      break;
    }
  }
}
module.exports = {validateCommand, planeAxes};
