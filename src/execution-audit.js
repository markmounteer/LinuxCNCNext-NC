"use strict";
const {invariant} = require("./internal-error");
// The existing reader/formatter treat +0 and -0 as the same coordinate. Keep
// that exact numeric contract without tolerances or JSON's NaN-to-null coercion.
function equal(a, b) {
  if (a === b) return true;
  if (!a || !b || typeof a !== "object" || typeof b !== "object" || Array.isArray(a) !== Array.isArray(b)) return false;
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every(k => Object.hasOwn(b, k) && equal(a[k], b[k]));
}
// Enumerate source USES, not unique geometry: repeated vertices and shared curves
// each remain required. This traversal does not consume the emitter's loop state.
function* expectedUses(program, plan) {
  function* waypoints(points, pointer, frame, section, stage) {
    for (const [i, target] of points.entries()) yield {type: "waypoint", pointer: `${pointer}/${i}`, frame, section, stage, waypoint: i + 1, target};
  }
  for (const [i, s] of program.model.sections.entries()) {
    const section = i + 1;
    yield {type: "section-start", section};
    const t = plan.sections[i], mode = t.mode || "retract";
    if (mode === "retract") {
      yield* waypoints(t.retract, `/sections/${i}/retract`, "machine", section, "retract");
      yield* waypoints(t.approach, `/sections/${i}/approach`, "work", section, "approach");
    } else if (mode === "link") yield* waypoints(t.moves, `/sections/${i}/moves`, "work", section, "link");
    let position = s.start;
    for (const [j, p] of s.paths.entries()) {
      const path = j + 1, common = {section, path};
      yield {type: "path-start", ...common};
      if (p.points) {
        for (let v = 1; v < p.points.length; v++) {
          yield {type: "path-use", ...common, segment: v, kind: p.kind, start: position, end: p.points[v], source: p};
          position = p.points[v];
        }
      } else if (p.kind === "arc") {
        const end = p.fullCircle ? position : p.end;
        yield {type: "path-use", ...common, kind: "arc", start: position, end, source: p};
        position = end;
      } else yield {type: "path-use", ...common, kind: "dwell", position, source: p};
      yield {type: "path-end", ...common};
    }
    yield {type: "section-end", section};
  }
  yield* waypoints(plan.end, "/end", "machine", undefined, "program-end");
}
function auditExecution(program, plan, output, ranges, boundaries) {
  const {lines, sourceMap} = output, expected = expectedUses(program, plan);
  let boundary = 0, checkedUses = 0;
  const summary = {status: "passed", sections: 0, paths: 0, segments: 0, arcs: 0, dwells: 0, planWaypoints: 0};
  const check = (ok, rule, message, context) => invariant(ok, rule, message, context);
  const axes = program.model.machine === "mill" ? [0, 1, 2] : [0, 2];
  const sourceFor = use => {
    const s = program.provenance.sections[(use?.section || 0) - 1];
    return s && {operation: s.operation, ...(use.path ? {toolpath: s.paths[use.path - 1].toolpath} : {})};
  };
  check(lines.length === sourceMap.length && lines.length > 0, "MAP_LENGTH", "Output and source-map lengths disagree.");
  function consume(actual, entry) {
    const next = expected.next(), want = next.value;
    const context = {line: entry?.line, section: want?.section ?? entry?.section, path: want?.path ?? entry?.path,
      segment: want?.segment, expectedUse: want && {type: want.type, kind: want.kind, pointer: want.pointer}, expectedProvenance: sourceFor(want), provenance: entry?.provenance};
    check(!next.done, "EXTRA_USE", "Output contains an unexpected source use.", context);
    check(actual.type === want.type && actual.section === want.section && actual.path === want.path,
      "USE_ORDER", "Missing, duplicated, reordered or misattributed source use.", context);
    if (want.type.endsWith("start") || want.type.endsWith("end")) {
      if (want.type === "section-start") ++summary.sections;
      if (want.type === "path-start") ++summary.paths;
      return;
    }
    ++checkedUses;
    const c = entry.command;
    if (want.type === "waypoint") {
      check(c.type === "rapid" && c.frame === want.frame && equal(c.axes, want.target) &&
        entry.stage === want.stage && entry.waypoint === want.waypoint &&
        entry.phase === (want.stage === "link" ? "link" : want.stage === "program-end" ? "program-end" : "transition") &&
        equal(entry.motion, {kind: "rapid", frame: want.frame, target: want.target}) &&
        entry.provenance?.origin === "execution-plan" && entry.provenance.pointer === want.pointer,
      "WAYPOINT_USE", "Reviewed transition waypoint disagrees with output or source mapping.", context);
      ++summary.planWaypoints; return;
    }
    const p = want.source, s = program.model.sections[want.section - 1], mapped = plan.tools[`${s.tool.number}:${s.tool.offset}`];
    const ss = program.provenance.sections[want.section - 1], ps = ss.paths[want.path - 1], provenance = entry.provenance;
    check(provenance?.origin === "step" && equal(provenance.operation, ss.operation) && equal(provenance.workingstep, ss.workingstep) && equal(provenance.toolpath, ps.toolpath),
      "SOURCE_PROVENANCE", "Output provenance disagrees with the owning source use.", context);
    check(entry.operation === s.name && entry.kind === p.kind && c.type === p.kind && c.frame === "work" &&
      entry.segment === want.segment, "PATH_USE", "Output motion disagrees with the decoded path use.", context);
    const modal = entry.modalState, speed = p.spindle.mode === "css" ? p.spindle.speed / (program.model.units === "mm" ? 1000 : 12) : p.spindle.speed;
    check(modal && modal.units === program.model.units && modal.tool === mapped.tool && modal.toolOffset === mapped.offset &&
      modal.workOffset === plan.workOffsets[s.workOffset] && modal.spindleMode === p.spindle.mode && modal.spindleSpeed === speed &&
      modal.spindleMaximumRPM === (p.spindle.mode === "css" ? p.spindle.maximumRPM : null) &&
      modal.spindleDirection === (p.spindle.clockwise ? "clockwise" : "counterclockwise") &&
      modal.coolant === p.coolant &&
      (!p.feed || modal.feedMode === p.feed.mode && modal.feedRate === p.feed.value),
    "PROCESS_STATE", "State at the source use disagrees with the validated process or mappings.", context);
    if (p.kind === "dwell") {
      check(entry.action === "dwell" && c.seconds === p.seconds && entry.seconds === p.seconds && equal(entry.position, want.position) && equal(provenance.property, ps.dwell),
        "DWELL_USE", "Dwell value/position disagrees with the source.", context);
      ++summary.dwells; return;
    }
    const m = entry.motion;
    check(entry.action === "motion" && m?.kind === p.kind && m.frame === "work" && equal(m.start, want.start) && equal(m.end, want.end),
      "MOTION_DETAIL", "Recorded motion start/end disagrees with the ordered source geometry.", context);
    if (p.kind === "arc") {
      const plane = p.plane || "XZ", active = {XY: [0, 1], XZ: [0, 2], YZ: [1, 2]}[plane];
      const center = Object.fromEntries(active.map(a => ["xyz"[a], p.center[a] - p.start[a]]));
      const endpoint = p.fullCircle ? {} : Object.fromEntries(active.map(a => ["xyz"[a], p.end[a]]));
      check(c.plane === plane && modal.plane === plane && equal(c.centerOffset, center) && equal(c.axes, endpoint) &&
        c.fullCircle === p.fullCircle && c.clockwise === p.clockwise && m.fullCircle === p.fullCircle && m.clockwise === p.clockwise &&
        m.plane === (program.model.machine === "mill" ? plane : undefined) && equal(m.sourceStart, p.start) && equal(m.sourceCenter, p.center) && equal(m.centerOffset, [center.x || 0, center.y || 0, center.z || 0]) && equal(provenance.curve, ps.curve) && equal(provenance.arc, ps.arc),
      "ARC_USE", "Arc command/details disagree with the source plane, center, sense or endpoint.", context);
      ++summary.arcs;
    } else {
      let changed = axes.filter(a => want.start[a] !== want.end[a]);
      if (!changed.length) changed = axes;
      check(equal(c.axes, Object.fromEntries(changed.map(a => ["xyz"[a], want.end[a]]))) &&
        entry.fromVertex === want.segment && entry.toVertex === want.segment + 1 && equal(provenance.curve, ps.curve) && equal(provenance.from, ps.vertices[want.segment - 1]) && equal(provenance.to, ps.vertices[want.segment]),
      "SEGMENT_USE", "Polyline command or vertex association disagrees with the source.", context);
      ++summary.segments;
    }
  }
  for (let i = 0; i <= lines.length; i++) {
    while (boundary < boundaries.length && boundaries[boundary].at === i) consume(boundaries[boundary++]);
    if (i === lines.length) break;
    const entry = sourceMap[i], c = entry.command;
    check(entry.line === i + 1 && c && typeof lines[i] === "string", "MAP_LINE", "Invalid output line association.", {line: i + 1});
    const isMotion = ["rapid", "linear", "arc"].includes(c.type), isDwell = c.type === "dwell";
    check(isMotion === (entry.action === "motion") && isDwell === (entry.action === "dwell"), "MAP_ACTION", "Command and source-map action disagree.", {line: i + 1, provenance: entry.provenance});
    if (isMotion || isDwell) consume({type: entry.phase === "toolpath" ? "path-use" : "waypoint", section: entry.section, path: entry.path}, entry);
  }
  const remaining = expected.next();
  check(boundary === boundaries.length && remaining.done, "INCOMPLETE_OUTPUT", "A required source use or boundary was not emitted.", {expectedProvenance: sourceFor(remaining.value), expectedUse: remaining.value && {type: remaining.value.type, section: remaining.value.section, path: remaining.value.path, pointer: remaining.value.pointer}});
  check(sourceMap.at(-1).command.type === "end" && sourceMap.filter(e => e.command.type === "end").length === 1 && output.state.ended,
    "PROGRAM_END", "Translation did not complete exactly one program end.");
  check(ranges.length === program.model.sections.length, "OPERATION_RANGES", "Operation range count disagrees with the input.");
  // One pass over the map, avoiding one scan per operation for this audit.
  const actualRanges = new Map();
  for (const e of sourceMap) if (e.section !== undefined) {
    if (!actualRanges.has(e.section)) actualRanges.set(e.section, {first: e.line, last: e.line, motions: 0});
    const r = actualRanges.get(e.section); r.last = e.line;
    if (e.phase === "toolpath" && e.action === "motion") ++r.motions;
  }
  ranges.forEach((r, i) => {
    const actual = actualRanges.get(i + 1), s = program.model.sections[i];
    check(actual && r.section === i + 1 && r.operation === s.name && r.firstLine === actual.first && r.lastLine === actual.last && r.toolpathMotions === actual.motions &&
      equal(r.tool, s.tool) && equal(r.mappedTool, plan.tools[`${s.tool.number}:${s.tool.offset}`]) && r.mappedWorkOffset === plan.workOffsets[s.workOffset],
      "OPERATION_RANGE", "Recorded operation line range disagrees with output.", {section: i + 1});
  });
  return {...summary, checkedUses, outputLines: lines.length, scope: "Ordered source/plan consumption and internal command-state consistency; not controller or machine acceptance."};
}
module.exports = {auditExecution};
