"use strict";
const {requireValue: need} = require("./errors");
const {validateCommand} = require("./command-contract");
const {invariant} = require("./internal-error");
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
const planes = {XY: "G17", XZ: "G18", YZ: "G19"};
const clone = value => JSON.parse(JSON.stringify(value));
const unknownPosition = () => [null, null, null];
// Each instance belongs to exactly one translation. State describes commands we
// emitted, not measured position, remap internals or offset-table contents.
class LinuxCNCOutput {
  constructor({machine = "lathe", units = null} = {}) {
    invariant(["lathe", "mill"].includes(machine) && (units === null || ["mm", "inch"].includes(units)), "OUTPUT_CONTEXT", "Invalid output machine/units.");
    this.machine = machine; this.units = units; this.started = false;
    this.lines = []; this.sourceMap = [];
    this.requested = {spindle: null, coolant: "off", feed: null, selectedTool: null};
    this.state = {units: null, plane: null, distanceMode: null, arcDistanceMode: null, diameterMode: null,
      cutterCompensation: null, cycle: null, feedMode: null, feedRate: null, motionControl: null,
      tool: null, toolOffset: null, workOffset: null, temporaryOffsets: null,
      spindleMode: null, spindleSpeed: null, spindleMaximumRPM: null, spindleDirection: null, coolant: null,
      workPosition: unknownPosition(), machinePosition: unknownPosition(), ended: false};
  }
  emit(command, source, detail = {}) {
    validateCommand(command, this, {...source, commandIndex: this.lines.length + 1, provenance: detail.provenance});
    const before = clone(this.state), next = clone(before), c = command;
    let line;
    const words = (axes, letters = "XYZ") => Object.entries(axes).map(([axis, value]) => ` ${letters["xyz".indexOf(axis)]}${decimal(value)}`).join("");
    switch (c.type) {
      case "comment": line = comment(c.text); break;
      case "initialize":
        line = (c.units === "mm" ? "G21" : "G20") + ` ${planes[c.plane]} G8 G90 G91.1 G40 G80 G94 G61`;
        Object.assign(next, {units: c.units, plane: c.plane, diameterMode: false, distanceMode: "absolute", arcDistanceMode: "incremental", cutterCompensation: "off", cycle: "off", feedMode: "perMinute", feedRate: 0, motionControl: "exactPath"}); break;
      case "clearTemporaryOffsets": line = "G92.1"; next.temporaryOffsets = "cleared"; next.workPosition = unknownPosition(); break;
      case "spindleStop": line = "M5 $0"; next.spindleDirection = "stopped"; break;
      case "spindleMode":
        line = c.mode === "css" ? `G96 D${decimal(c.maximumRPM)} S${decimal(c.speed)} $0` : `G97 S${decimal(c.speed)} $0`;
        Object.assign(next, {spindleMode: c.mode, spindleSpeed: c.speed, spindleMaximumRPM: c.mode === "css" ? c.maximumRPM : null}); break;
      case "spindleStart": line = c.clockwise ? "M3 $0" : "M4 $0"; next.spindleDirection = c.clockwise ? "clockwise" : "counterclockwise"; break;
      case "coolant": line = {off: "M9", flood: "M8", mist: "M7"}[c.value]; next.coolant = c.value; break;
      case "feed": line = (c.mode === "perRevolution" ? "G95" : "G94") + (c.value === undefined ? "" : ` F${decimal(c.value)}`); next.feedMode = c.mode; next.feedRate = c.value === undefined ? 0 : c.value; break;
      case "cancelToolOffset": line = "G49"; next.toolOffset = 0; next.workPosition = unknownPosition(); break;
      case "toolChange":
        line = `T${c.tool} M6`;
        // A custom M6 can move and alter modes; the subsequent explicit
        // commands establish what is known again. Never invent its position.
        for (const key of Object.keys(next)) next[key] = null;
        Object.assign(next, {tool: c.tool, workPosition: unknownPosition(), machinePosition: unknownPosition(), ended: false}); break;
      case "workOffset": line = c.value; next.workOffset = c.value; next.workPosition = unknownPosition(); break;
      case "toolOffset": line = `G43 H${c.offset}`; next.toolOffset = c.offset; next.workPosition = unknownPosition(); break;
      case "plane": line = planes[c.value]; next.plane = c.value; break;
      case "rapid": case "linear":
        line = (c.frame === "machine" ? "G53 " : "") + (c.type === "rapid" ? "G0" : "G1") + words(c.axes); break;
      case "arc": line = (c.clockwise ? "G2" : "G3") + words(c.axes) + words(c.centerOffset, "IJK") + (c.fullCircle ? " P1" : ""); break;
      case "dwell": line = `G4 P${decimal(c.seconds)}`; break;
      case "end":
        line = "M2";
        // The controller performs its program-end reset. Our commanded-state
        // model ends here rather than retaining a misleading pre-M2 plane/WCS.
        for (const key of Object.keys(next)) next[key] = null;
        Object.assign(next, {workPosition: unknownPosition(), machinePosition: unknownPosition(), ended: true}); break;
      default: throw new TypeError("Unknown internal machining command: " + c.type);
    }
    need(typeof line === "string" && line.length <= 240, "LINE_LENGTH", "Generated line exceeds 240 characters.", source);
    if (["rapid", "linear", "arc"].includes(c.type)) {
      const frame = c.frame === "machine" ? "machinePosition" : "workPosition", other = c.frame === "machine" ? "workPosition" : "machinePosition";
      for (const [axis, value] of Object.entries(c.axes)) { const i = "xyz".indexOf(axis); next[frame][i] = value; next[other][i] = null; }
    }
    const changed = Object.keys(next).filter(key => JSON.stringify(before[key]) !== JSON.stringify(next[key]));
    const stateChange = Object.fromEntries(changed.map(key => [key, {before: before[key], after: next[key]}]));
    const entry = {line: this.lines.length + 1, action: "state", ...source, ...detail,
      ...(detail.provenance ? {provenance: clone(detail.provenance)} : {}),
      gcode: line, command: clone({frame: "modal", ...c}), stateChange,
      ...(["rapid", "linear", "arc", "dwell"].includes(c.type) ? {modalState: Object.fromEntries(["units", "plane", "feedMode", "feedRate", "spindleMode", "spindleSpeed", "spindleMaximumRPM", "spindleDirection", "coolant", "tool", "toolOffset", "workOffset"].map(key => [key, next[key]]))} : {})};
    if (c.type === "initialize") { this.started = true; this.units = c.units; }
    this.state = next; this.lines.push(line); this.sourceMap.push(entry);
  }
}
module.exports = {LinuxCNCOutput, decimal, comment};
