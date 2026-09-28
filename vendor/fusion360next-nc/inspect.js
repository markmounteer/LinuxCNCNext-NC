"use strict";
// Decode and check this project's narrow execution profile independently of the
// writer. This is neither a general AP238 validator nor a machine interpreter.
const {parse} = require("./part21");
const {createHash} = require("node:crypto");
const PROFILE = "next-nc/turning-toolpath/0.1";
function check(ok, message) { if (!ok) throw new Error("Next-NC inspection: " + message); }
function positive(n, label) { check(Number.isFinite(n) && n > 0, label + " must be positive"); return n; }
function close(a, b, epsilon = 1e-9) { return a.length === b.length && a.every((v, i) => Math.abs(v - b[i]) <= epsilon); }
function inspect(text) {
  check(text.includes("FILE_SCHEMA(('INTEGRATED_CNC_SCHEMA'))"), "unsupported schema");
  const doc = parse(text), types = new Map(), properties = new Map(), links = new Map();
  for (const [id, parts] of doc.records) for (const part of parts) {
    const entry = {id, ...part};
    if (!types.has(part.type)) types.set(part.type, []);
    types.get(part.type).push(entry);
  }
  const all = type => types.get(type) || [];
  function get(ref, type) {
    const parts = ref && doc.records.get(ref.ref);
    const candidates = (parts || []).filter(p => p.type === type);
    check(candidates.length === 1, `expected ${type} at #${ref && ref.ref}`);
    return candidates[0].args;
  }
  function one(items, label) { check(items.length === 1, "expected one " + label); return items[0]; }
  for (const p of all("ACTION_PROPERTY_REPRESENTATION")) {
    const id = p.args[2].ref;
    check(!links.has(id), "duplicate property representation"); links.set(id, p.args[3]);
  }
  for (const p of all("ACTION_PROPERTY")) {
    const key = p.args[2].ref + "|" + p.args[0];
    check(!properties.has(key) && links.has(p.id), "duplicate or unrepresented property " + p.args[0]);
    properties.set(key, links.get(p.id));
  }
  function prop(id, name, type = "REPRESENTATION", optional = false) {
    const ref = properties.get(id + "|" + name);
    if (!ref && optional) return null;
    check(ref, `missing ${name} on #${id}`); return get(ref, type);
  }
  function textProp(id, name) { return get(one(prop(id, name)[1], name), "DESCRIPTIVE_REPRESENTATION_ITEM")[1]; }
  function relationship(type, id) { return all(type).filter(e => e.args[2].ref === id); }
  function target(type, id) { return one(relationship(type, id), type + " for #" + id).args[3]; }
  const visited = {MACHINING_WORKINGSTEP: new Set(), TURNING_TYPE_OPERATION: new Set(), MACHINING_TOOLPATH: new Set()};
  function visit(ref, type) {
    check(!visited[type].has(ref.ref), "repeated " + type); visited[type].add(ref.ref); return get(ref, type);
  }
  function sequence(type, id) {
    const rows = relationship(type, id).sort((a, b) => a.args[4] - b.args[4]);
    check(rows.length > 0 && rows.every((row, i) => row.args[4] === i + 1), "invalid or empty sequence on #" + id);
    return rows.map(row => row.args[3]);
  }
  const unitCache = new Map(), activeUnits = new Set();
  function unit(ref) {
    if (unitCache.has(ref.ref)) return unitCache.get(ref.ref);
    check(!activeUnits.has(ref.ref), "cyclic unit definition"); activeUnits.add(ref.ref);
    const parts = doc.records.get(ref.ref), has = type => parts.some(p => p.type === type);
    let result;
    if (has("SI_UNIT")) {
      const [prefix, name] = get(ref, "SI_UNIT").map(v => v.symbol);
      if (has("LENGTH_UNIT") && prefix === ".MILLI." && name === ".METRE.") result = "mm";
      if (has("TIME_UNIT") && prefix === "$" && name === ".SECOND.") result = "second";
      if (has("PLANE_ANGLE_UNIT") && prefix === "$" && name === ".RADIAN.") result = "radian";
      if (has("SOLID_ANGLE_UNIT") && prefix === "$" && name === ".STERADIAN.") result = "steradian";
    } else if (has("CONVERSION_BASED_UNIT")) {
      const [name, factor] = get(ref, "CONVERSION_BASED_UNIT");
      const length = name === "inch", time = name === "minute";
      check((length && has("LENGTH_UNIT")) || (time && has("TIME_UNIT")), "unsupported conversion unit");
      const measure = get(factor, length ? "LENGTH_MEASURE_WITH_UNIT" : "TIME_MEASURE_WITH_UNIT");
      check(measure[0].type === (length ? "LENGTH_MEASURE" : "TIME_MEASURE") &&
        measure[0].args[0] === (length ? 25.4 : 60) && unit(measure[1]) === (length ? "mm" : "second"), "invalid unit conversion");
      const dims = get(get(ref, "NAMED_UNIT")[0], "DIMENSIONAL_EXPONENTS");
      check(close(dims, length ? [1, 0, 0, 0, 0, 0, 0] : [0, 0, 1, 0, 0, 0, 0], 0), "invalid unit dimensions");
      result = name;
    } else if (has("CONTEXT_DEPENDENT_UNIT")) {
      const [dimensions, name] = get(ref, "CONTEXT_DEPENDENT_UNIT");
      check(name === "revolution" && close(get(dimensions, "DIMENSIONAL_EXPONENTS"), [0, 0, 0, 0, 0, 0, 0], 0), "invalid revolution unit");
      result = name;
    } else if (has("DERIVED_UNIT")) {
      const elements = get(ref, "DERIVED_UNIT")[0].map(r => get(r, "DERIVED_UNIT_ELEMENT"));
      check(elements.length === 2, "invalid derived unit");
      const numerator = one(elements.filter(e => e[1] === 1), "unit numerator");
      const denominator = one(elements.filter(e => e[1] === -1), "unit denominator");
      result = unit(numerator[0]) + "/" + unit(denominator[0]);
    }
    check(result, "unsupported unit at #" + ref.ref); activeUnits.delete(ref.ref); unitCache.set(ref.ref, result); return result;
  }
  const context = one(all("GLOBAL_UNIT_ASSIGNED_CONTEXT"), "geometry unit context");
  check(get({ref: context.id}, "GEOMETRIC_REPRESENTATION_CONTEXT")[0] === 3, "expected 3D geometry");
  const geometryUnits = context.args[0].map(unit), units = geometryUnits.find(u => u === "mm" || u === "inch");
  check(units && geometryUnits.length === 3 && new Set(geometryUnits).size === 3 &&
    geometryUnits.includes("radian") && geometryUnits.includes("steradian"), "invalid geometry units");
  function geometry(rep) { check(rep[2].ref === context.id, "geometry has a different unit context"); return one(rep[1], "geometry item"); }
  function measure(ref, expectedUnit, valueType = "NUMERIC_MEASURE") {
    const args = get(ref, "MEASURE_REPRESENTATION_ITEM");
    check(args[1].type === valueType && args[1].args.length === 1 && Number.isFinite(args[1].args[0]), "invalid measure");
    check(unit(args[2]) === expectedUnit, "incorrect measure units: expected " + expectedUnit); return args[1].args[0];
  }
  function processState(id) {
    const tech = target("MACHINING_TECHNOLOGY_RELATIONSHIP", id).ref;
    check(get({ref: tech}, "MACHINING_TECHNOLOGY")[1] === "turning", "unsupported technology");
    check(textProp(tech, "feedrate reference") === "tool center point", "unsupported feed reference");
    const sr = prop(tech, "spindle", "MACHINING_SPINDLE_SPEED_REPRESENTATION");
    const css = sr[0] === "cutting speed";
    check(css || sr[0] === "spindle speed", "unsupported spindle mode");
    check(sr[1].length === (css ? 2 : 1), "incorrect spindle measures");
    const signedSpeed = measure(sr[1][0], css ? units + "/minute" : "revolution/minute");
    const spindle = {mode: css ? "css" : "rpm", speed: positive(Math.abs(signedSpeed), "spindle speed"), clockwise: signedSpeed < 0};
    if (css) spindle.maximumRPM = positive(measure(sr[1][1], "revolution/minute"), "CSS maximum RPM");
    const fr = prop(tech, "feedrate", "MACHINING_FEED_SPEED_REPRESENTATION", true);
    let feed = null;
    if (fr) {
      const perRev = fr[0] === "feed per revolution";
      check(perRev || fr[0] === "feed speed", "unsupported feed mode");
      feed = {value: positive(measure(one(fr[1], "feed measure"), units + (perRev ? "/revolution" : "/minute")), "feed"), mode: perRev ? "perRevolution" : "perMinute"};
    }
    const functions = target("MACHINING_FUNCTIONS_RELATIONSHIP", id).ref;
    check(get({ref: functions}, "MACHINING_FUNCTIONS")[1] === "turning", "unsupported machine functions");
    const c = textProp(functions, "coolant");
    check(c === "coolant off" || c === "coolant on", "unsupported coolant state");
    const coolant = c === "coolant off" ? "off" : textProp(functions, "coolant type");
    check(["off", "flood", "mist", "through tool"].includes(coolant), "unsupported coolant type");
    return {spindle, feed, coolant};
  }
  function point(ref) {
    const p = get(ref, "CARTESIAN_POINT")[1];
    check(p.length === 3 && p.every(Number.isFinite) && p[1] === 0, "invalid XZ point"); return p;
  }
  function vector(ref) {
    const v = get(ref, "DIRECTION")[1];
    check(v.length === 3 && v.every(Number.isFinite) && Math.abs(Math.hypot(...v) - 1) < 1e-9, "invalid unit direction"); return v;
  }
  const report = {validation: "Next-NC profile checks only; not AP238 certification or machine validation", units,
    entities: doc.records.size, sections: 0, paths: 0, rapidPaths: 0, linearPaths: 0, arcs: 0, dwells: 0,
    rapidSegments: 0, cuttingSegments: 0, bounds: {min: [Infinity, 0, Infinity], max: [-Infinity, 0, -Infinity]},
    maxRadialMismatch: 0, cartesianPointRecords: all("CARTESIAN_POINT").length,
    distinctCartesianPoints: new Set(all("CARTESIAN_POINT").map(p => JSON.stringify(p.args[1]))).size,
    curveDefinitions: {polylines: all("POLYLINE").length, arcs: all("TRIMMED_CURVE").length}, operations: []};
  function bounds(p) { for (const i of [0, 2]) { report.bounds.min[i] = Math.min(report.bounds.min[i], p[i]); report.bounds.max[i] = Math.max(report.bounds.max[i], p[i]); } }
  const tau = 2 * Math.PI, wrap = a => (a % tau + tau) % tau;
  function arc(ref) {
    const trim = get(ref, "TRIMMED_CURVE"), circle = get(trim[1], "CIRCLE"), axis = get(circle[1], "AXIS2_PLACEMENT_3D");
    const center = point(axis[1]), normal = vector(axis[2]), direction = vector(axis[3]), radius = positive(circle[2], "arc radius");
    check(close(normal, [0, 1, 0], 0) && direction[1] === 0, "arc is not in the +Y XZ frame");
    check([".T.", ".F."].includes(trim[4].symbol), "arc direction is not explicit");
    const clockwise = trim[4].symbol === ".F.", fullCircle = trim[5].symbol === ".PARAMETER.";
    const first = one(trim[2], "first trim"), last = one(trim[3], "last trim");
    let start, end;
    if (fullCircle) {
      check(first.type === "PARAMETER_VALUE" && first.args[0] === 0 && last.type === "PARAMETER_VALUE" && last.args[0] === tau, "unsupported full circle trim");
      start = center.map((v, i) => v + radius * direction[i]); end = start.slice();
    } else {
      check(trim[5].symbol === ".CARTESIAN.", "unsupported arc trim"); start = point(first); end = point(last);
      check(!close(start, end, 0), "partial arc has coincident endpoints");
    }
    for (const p of [start, end]) {
      const mismatch = Math.abs(Math.hypot(p[0] - center[0], p[2] - center[2]) - radius);
      report.maxRadialMismatch = Math.max(report.maxRadialMismatch, mismatch);
      check(mismatch <= Math.max(1e-7, radius * 1e-6), "arc endpoint radius mismatch"); bounds(p);
    }
    check(close(direction, start.map((v, i) => (v - center[i]) / radius), 1e-8), "arc reference direction disagrees with start");
    const a = Math.atan2(-(start[2] - center[2]), start[0] - center[0]);
    const b = Math.atan2(-(end[2] - center[2]), end[0] - center[0]);
    const sweep = fullCircle ? tau : wrap(clockwise ? a - b : b - a);
    for (let i = 0; i < 4; ++i) {
      const angle = i * Math.PI / 2, along = wrap(clockwise ? a - angle : angle - a);
      if (along <= sweep + 1e-12) bounds([center[0] + radius * [1, 0, -1, 0][i], 0, center[2] - radius * [0, 1, 0, -1][i]]);
    }
    return {kind: "arc", start, end, center, radius, clockwise, fullCircle};
  }
  function nonnegativeInteger(text, label, minimum = 0) {
    check(typeof text === "string" && /^\d+$/.test(text) && Number.isSafeInteger(Number(text)) && Number(text) >= minimum, "invalid " + label); return Number(text);
  }
  const plan = one(all("MACHINING_WORKPLAN"), "workplan");
  one(all("MACHINING_PROJECT"), "project");
  check(textProp(plan.id, "next-nc profile") === PROFILE, "unsupported Next-NC profile");
  check(textProp(plan.id, "next-nc coordinates") === "WCS; X radius; Y zero; Z axial; Fusion tool reference point", "unsupported coordinate convention");
  const model = {name: plan.args[0], units, sections: []};
  for (const ws of sequence("MACHINING_PROCESS_SEQUENCE_RELATIONSHIP", plan.id)) {
    const step = visit(ws, "MACHINING_WORKINGSTEP"), operation = target("MACHINING_OPERATION_RELATIONSHIP", ws.ref);
    const op = visit(operation, "TURNING_TYPE_OPERATION"), id = operation.ref;
    check(step[0] === op[0], "workingstep/operation names differ");
    const tool = one(all("MACHINING_TOOL").filter(t => t.args[2].some(r => r.ref === id)), "tool for " + op[0]);
    check(get(tool.args[3], "ACTION_RESOURCE_TYPE")[0] === "cutting tool", "unsupported tool type");
    const initial = processState(id), start = point(geometry(prop(id, "next-nc entry point")));
    const section = {name: op[0], tool: {number: nonnegativeInteger(tool.args[0], "tool number", 1),
      offset: nonnegativeInteger(textProp(id, "next-nc tool offset"), "tool offset"), description: tool.args[1]},
      workOffset: nonnegativeInteger(textProp(id, "next-nc work offset"), "work offset"), start,
      initialSpindle: initial.spindle, initialCoolant: initial.coolant, paths: []};
    let position = start; bounds(start);
    for (const pathRef of sequence("MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP", id)) {
      const path = visit(pathRef, "MACHINING_TOOLPATH"), pid = pathRef.ref, state = processState(pid);
      check(textProp(pid, "priority") === "required", "unsupported path priority");
      let decoded;
      if (path[1] === "feedstop") {
        decoded = {kind: "dwell", seconds: positive(measure(one(prop(pid, "dwell")[1], "dwell"), "second", "TIME_MEASURE"), "dwell")}; ++report.dwells;
      } else {
        check(path[1] === "cutter location trajectory" && textProp(pid, "trajectory type") === "trajectory path" &&
          textProp(pid, "direction") === "beginning to end", "unsupported trajectory");
        const curve = geometry(prop(pid, "basic curve")), speed = prop(pid, "speed profile", "MACHINING_TOOLPATH_SPEED_PROFILE_REPRESENTATION", true);
        const rapid = Boolean(speed);
        if (speed) check(get(one(speed[1], "rapid speed"), "DESCRIPTIVE_REPRESENTATION_ITEM")[1] === "rapid", "unsupported speed profile");
        if (doc.records.get(curve.ref).some(e => e.type === "POLYLINE")) {
          const points = get(curve, "POLYLINE")[1].map(point);
          check(points.length >= 2, "polyline needs two points"); points.forEach(bounds);
          decoded = {kind: rapid ? "rapid" : "linear", points};
          ++report[rapid ? "rapidPaths" : "linearPaths"]; report[rapid ? "rapidSegments" : "cuttingSegments"] += points.length - 1;
        } else { check(!rapid, "rapid arcs are unsupported"); decoded = arc(curve); ++report.arcs; }
        check(rapid ? state.feed === null : state.feed !== null, rapid ? "rapid has cutting feed" : "cutting path has no feed");
        check(close(position, decoded.start || decoded.points[0]), `path discontinuity in ${op[0]} at #${pid}`);
        position = decoded.end || decoded.points[decoded.points.length - 1];
      }
      section.paths.push({...decoded, ...state}); ++report.paths;
    }
    model.sections.push(section);
    report.operations.push({name: section.name, tool: section.tool.number, toolOffset: section.tool.offset,
      workOffset: section.workOffset, paths: section.paths.length, arcs: section.paths.filter(p => p.kind === "arc").length,
      spindle: section.initialSpindle, feeds: [...new Map(section.paths.filter(p => p.feed).map(p => [JSON.stringify(p.feed), p.feed])).values()]});
  }
  for (const [type, ids] of Object.entries(visited)) check(ids.size === all(type).length, "orphan " + type);
  report.sections = model.sections.length;
  // Stable ordered model, exact decoded numbers, independent of STEP IDs,
  // shared definitions, timestamp and writer release. This is a fingerprint of
  // our decoded subset, not a signature of all possible AP238 semantics.
  const schema = "next-nc/decoded-program/1";
  report.programFingerprint = {schema, algorithm: "sha256",
    value: createHash("sha256").update(JSON.stringify({schema, model})).digest("hex")};
  return {model, report};
}
function firstDifference(before, after, location = "program") {
  if (before === after) return null;
  if (before && after && typeof before === "object" && typeof after === "object" && Array.isArray(before) === Array.isArray(after)) {
    if (Array.isArray(before) && before.length !== after.length) return {path: location + ".length", before: before.length, after: after.length};
    for (const key of new Set([...Object.keys(before), ...Object.keys(after)])) {
      const difference = firstDifference(before[key], after[key], Array.isArray(before) ? `${location}[${key}]` : `${location}.${key}`);
      if (difference) return difference;
    }
    return null;
  }
  return {path: location, before: before === undefined ? null : before, after: after === undefined ? null : after};
}
function compare(beforeText, afterText) {
  // Validate both complete inputs; a valid-looking hash cannot bypass checks.
  const before = inspect(beforeText), after = inspect(afterText);
  const difference = firstDifference(before.model, after.model);
  return {sameProgram: difference === null, beforeFingerprint: before.report.programFingerprint,
    afterFingerprint: after.report.programFingerprint, firstDifference: difference};
}
module.exports = {inspect, compare};
