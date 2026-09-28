"use strict";
function exitPoint(section) {
  let point = section.start;
  // A full circle returns to the commanded start, even if STEP's reconstructed
  // circle point differs within the reader's floating-point join tolerance.
  for (const path of section.paths) if (path.kind !== "dwell" && !(path.kind === "arc" && path.fullCircle)) point = path.end || path.points[path.points.length - 1];
  return point;
}
function continuation(program, index, plan) {
  const current = program.model.sections[index], previous = program.model.sections[index - 1];
  const no = reason => ({eligible: false, reason});
  if (!previous) return no("First operation requires a reviewed machine approach.");
  if (["number", "offset", "description"].some(k => previous.tool[k] !== current.tool[k])) return no("Fusion tool or tool offset differs.");
  if (previous.workOffset !== current.workOffset) return no("Fusion work offset differs.");
  if (exitPoint(previous).some((n, axis) => n !== current.start[axis])) return no("Previous exit and next entry differ; no connecting move is inferred.");
  const last = previous.paths[previous.paths.length - 1];
  const spindle = last ? last.spindle : previous.initialSpindle;
  if (["mode", "speed", "clockwise", "maximumRPM"].some(k => spindle[k] !== current.initialSpindle[k])) return no("Spindle mode, speed, direction or RPM cap differs.");
  if ((last ? last.coolant : previous.initialCoolant) !== current.initialCoolant) return no("Coolant state differs.");
  if (plan) {
    const a = plan.tools[`${previous.tool.number}:${previous.tool.offset}`], b = plan.tools[`${current.tool.number}:${current.tool.offset}`];
    if (!a || !b || a.tool !== b.tool || a.offset !== b.offset || plan.workOffsets[previous.workOffset] !== plan.workOffsets[current.workOffset]) return no("Mapped LinuxCNC tool, H offset or WCS differs.");
  }
  return {eligible: true, reason: "Exact entry/exit, tool, offsets, spindle and coolant match; no boundary motion is needed."};
}
module.exports = {exitPoint, continuation};
