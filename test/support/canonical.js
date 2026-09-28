"use strict";
const assert = require("node:assert/strict");
// LinuxCNC sai prints four decimal places. This harness compares that trace,
// never uses it to validate sub-0.0001 geometry or round production coordinates.
const TOLERANCE = 0.00011;
function canonicalEvents(trace) {
  return [...trace.matchAll(/^\s*\d+\s+N[.\d]+\s+([A-Z_]+)\((.*)\)\s*$/gm)]
    .filter(m => m[1] !== "COMMENT")
    .map(m => ({type: m[1], args: m[2].trim() ? m[2].trim().split(/[,\s]+/).map(s => /^[-+]?\d+(?:\.\d+)?$/.test(s) ? Number(s) : s) : []}));
}
function compareEvents(actual, expected) {
  assert.ok(expected.length > 0, "Expected a nonempty independent interpreter trace");
  assert.equal(actual.length, expected.length, "Canonical event count");
  actual.forEach((event, i) => {
    assert.equal(event.type, expected[i].type, `Canonical event ${i + 1}`);
    assert.equal(event.args.length, expected[i].args.length, `Canonical arguments ${i + 1}`);
    event.args.forEach((v, a) => typeof v === "number" && typeof expected[i].args[a] === "number" ?
      assert.ok(Math.abs(v - expected[i].args[a]) <= TOLERANCE, `Canonical ${event.type} event ${i + 1} argument ${a + 1}: ${v} != ${expected[i].args[a]}`) :
      assert.equal(v, expected[i].args[a], `Canonical ${event.type} argument ${a + 1}`));
  });
}
function assertSourceAssociations(program, result) {
  const expected = [];
  program.model.sections.forEach((s, section) => s.paths.forEach((p, path) => {
    const common = {section: section + 1, operation: s.name, path: path + 1, kind: p.kind};
    if (p.points) for (let i = 1; i < p.points.length; i++) expected.push({...common, segment: i, end: p.points[i]});
    else expected.push({...common, ...(p.kind === "arc" ? {end: p.end} : {seconds: p.seconds})});
  }));
  const actual = result.sourceMap.filter(e => e.phase === "toolpath" && ["motion", "dwell"].includes(e.action)).map(e => ({section: e.section, operation: e.operation, path: e.path, kind: e.kind,
    ...(e.segment ? {segment: e.segment} : {}), ...(e.motion ? {end: e.motion.end} : {seconds: e.seconds})}));
  assert.deepEqual(actual, expected, "Decoded path/segment association");
}
module.exports = {canonicalEvents, compareEvents, assertSourceAssociations, TOLERANCE};
