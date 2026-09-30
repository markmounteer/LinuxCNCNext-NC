"use strict";
const {requireValue: need} = require("./errors");
const finite = n => Number.isFinite(n) && Math.abs(n) < 1e15;
const point = p => Array.isArray(p) && p.length === 3 && p.every(finite);
const planes = {XY: [0, 1], XZ: [2, 0], YZ: [1, 2]}; // right-handed LinuxCNC G17/G18/G19 bases
const empty = () => ({cuttingDistance: 0, rapidDistance: 0, unknownDistanceBlocks: 0, dwellSeconds: 0, idealFeedSeconds: 0, unknownFeedTimeBlocks: 0, motionBlocks: 0});
function arcGeometry(start, end, offset, plane, clockwise, fullCircle) {
  need(point(start) && point(end) && point(offset) && planes[plane] && typeof clockwise === "boolean", "REVIEW_GEOMETRY", "Invalid archived arc geometry.");
  const axes = planes[plane], center = start.map((n, i) => n + offset[i]);
  const angle = p => Math.atan2(p[axes[1]] - center[axes[1]], p[axes[0]] - center[axes[0]]);
  const startAngle = angle(start), endAngle = angle(end), tau = 2 * Math.PI;
  const radius = Math.hypot(start[axes[0]] - center[axes[0]], start[axes[1]] - center[axes[1]]);
  need(radius > 0 && finite(radius), "REVIEW_GEOMETRY", "Invalid archived arc radius.");
  let sweep = fullCircle ? tau : ((clockwise ? startAngle - endAngle : endAngle - startAngle) % tau + tau) % tau;
  if (clockwise) sweep = -sweep;
  return {axes, center, radius, startAngle, sweep};
}
// Pure function also embedded in the offline browser reviewer.
function pointAt(segment, t) {
  if (t <= 0) return segment.start.slice();
  if (t >= 1) return segment.end.slice();
  if (!segment.arc) return segment.start.map((v, i) => v + (segment.end[i] - v) * t);
  const a = segment.arc, p = segment.start.slice(), angle = a.startAngle + a.sweep * t;
  p[a.axes[0]] = a.center[a.axes[0]] + a.radius * Math.cos(angle);
  p[a.axes[1]] = a.center[a.axes[1]] + a.radius * Math.sin(angle);
  return p;
}
function reviewGeometry(inspection, sourceMap, {includeRecords = true} = {}) {
  need(inspection && ["mm", "inch"].includes(inspection.units), "REVIEW_GEOMETRY", "Review requires recorded program units.");
  const machine = inspection.machine || "lathe";
  need(["lathe", "mill"].includes(machine) && Array.isArray(sourceMap) && (!includeRecords || sourceMap.length <= 200000), "REVIEW_GEOMETRY", "Interactive review requires a bounded source map (at most 200000 lines).");
  const totals = empty(), operations = new Map(), frames = new Map(), records = [];
  let workPosition = [null, null, null], machinePosition = [null, null, null];
  const retainedState = {};
  const normalize = p => Array.isArray(p) && p.length === 3 ? p.map((v, i) => machine === "lathe" && i === 1 ? 0 : finite(v) ? v : null) : [null, machine === "lathe" ? 0 : null, null];
  for (const [index, entry] of sourceMap.entries()) {
    need(entry && entry.line === index + 1 && entry.command && typeof entry.command.type === "string", "REVIEW_GEOMETRY", "Source-map lines must be complete and ordered with typed commands.");
    const c = entry.command, m = entry.motion;
    for (const [key, change] of Object.entries(entry.stateChange || {})) retainedState[key] = change.after;
    const state = entry.modalState || {...retainedState};
    const beforeWork = normalize(workPosition), beforeMachine = normalize(machinePosition);
    if (entry.stateChange?.workPosition) workPosition = normalize(entry.stateChange.workPosition.after);
    if (entry.stateChange?.machinePosition) machinePosition = normalize(entry.stateChange.machinePosition.after);
    const section = entry.section || 0;
    if (!operations.has(section)) operations.set(section, {section, operation: entry.operation || (section ? `Operation ${section}` : "Program policy"), ...empty()});
    const r = {line: entry.line, gcode: entry.gcode, section, operation: entry.operation || "Program policy", phase: entry.phase, stage: entry.stage, path: entry.path,
      kind: c.type, command: c, state, provenance: entry.provenance, distance: null, idealFeedSeconds: null};
    if (["rapid", "linear", "arc"].includes(c.type)) {
      need(m && ["work", "machine"].includes(m.frame), "REVIEW_GEOMETRY", "Motion is missing its coordinate frame.");
      r.frame = m.frame === "machine" ? "machine" : `work:${state.workOffset || "unknown"}`;
      r.start = m.start ? normalize(m.start) : m.frame === "machine" ? beforeMachine : beforeWork;
      r.end = m.end ? normalize(m.end) : normalize(m.frame === "machine" ? machinePosition : workPosition);
      if (c.type === "arc") {
        r.arc = arcGeometry(r.start, r.end, m.centerOffset, m.plane || (machine === "lathe" ? "XZ" : state.plane), m.clockwise, m.fullCircle);
        r.distance = r.arc.radius * Math.abs(r.arc.sweep);
      } else {
        const movedAxes = Object.keys(c.axes || {}).map(axis => "xyz".indexOf(axis));
        need(movedAxes.length && movedAxes.every(i => i >= 0), "REVIEW_GEOMETRY", "Archived motion has invalid axes.");
        if (movedAxes.every(i => finite(r.start[i]) && finite(r.end[i]))) r.distance = Math.hypot(...movedAxes.map(i => r.end[i] - r.start[i]));
      }
      r.drawable = point(r.start) && point(r.end);
      if (!frames.has(r.frame)) frames.set(r.frame, {frame: r.frame, ...empty()});
      if (c.type !== "rapid" && r.distance !== null && state.feedRate > 0) {
        if (state.feedMode === "perMinute") r.idealFeedSeconds = r.distance / state.feedRate * 60;
        else if (state.feedMode === "perRevolution" && state.spindleMode === "rpm" && state.spindleSpeed > 0 && ["clockwise", "counterclockwise"].includes(state.spindleDirection)) r.idealFeedSeconds = r.distance / (state.feedRate * state.spindleSpeed) * 60;
        if (!finite(r.idealFeedSeconds)) r.idealFeedSeconds = null;
      }
      for (const target of [totals, operations.get(section), frames.get(r.frame)]) {
        target.motionBlocks++;
        if (r.distance === null) target.unknownDistanceBlocks++;
        else target[c.type === "rapid" ? "rapidDistance" : "cuttingDistance"] += r.distance;
        if (c.type !== "rapid") {
          if (r.idealFeedSeconds === null) target.unknownFeedTimeBlocks++;
          else target.idealFeedSeconds += r.idealFeedSeconds;
        }
      }
    } else if (c.type === "dwell") {
      need(finite(c.seconds) && c.seconds >= 0, "REVIEW_GEOMETRY", "Invalid archived dwell.");
      r.seconds = c.seconds; r.end = normalize(entry.position);
      r.frame = `work:${state.workOffset || "unknown"}`;
      totals.dwellSeconds += c.seconds; operations.get(section).dwellSeconds += c.seconds;
      if (!frames.has(r.frame)) frames.set(r.frame, {frame: r.frame, ...empty()});
      frames.get(r.frame).dwellSeconds += c.seconds;
    }
    if (includeRecords) records.push(r);
  }
  return {schema: "linuxcnc-next-nc/review-geometry/1", machine, units: inspection.units,
    gcodeSHA256: inspection.traceability?.gcodeSHA256, programFingerprint: inspection.programFingerprint?.value,
    totals, operations: [...operations.values()], frames: [...frames.values()], records,
    limitations: ["Distances cover known commanded segments only; unknown starting coordinates are excluded and counted.",
      "Machine coordinates and each work offset are displayed separately; no offset transforms are inferred.",
      "Ideal feed time excludes rapid motion, acceleration, tool changes and spindle delays. G95 with CSS is not estimated.",
      "Review describes the archived candidate, not measured motion or physical clearance."]};
}
function motionSummary(inspection, sourceMap) {
  const {records, ...summary} = reviewGeometry(inspection, sourceMap, {includeRecords: false});
  return {...summary, schema: "linuxcnc-next-nc/motion-summary/1"};
}
module.exports = {reviewGeometry, motionSummary, arcGeometry, pointAt};
