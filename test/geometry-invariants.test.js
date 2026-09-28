"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const {geometryFixture, decodedGeometry, configurations, frames} = require("./support/geometry-fixture");
const {translate} = require("../src/translate"), {readProgram} = require("../src/profile"), {simulationPlan} = require("../scripts/example");
const {Program} = require("../vendor/fusion360next-nc/next-nc");
function equivalent(a, b) {
  if (typeof a === "number" && typeof b === "number") { assert.ok(Math.abs(a - b) < 1e-10, `${a} != ${b}`); return; }
  if (a && b && typeof a === "object" && typeof b === "object") { assert.deepEqual(Object.keys(a), Object.keys(b)); for (const k of Object.keys(a)) equivalent(a[k], b[k]); return; }
  assert.equal(a, b);
}
for (const [machine, plane] of configurations) for (const shift of [[20,20],[-20,20],[-20,-20],[20,-20]]) {
  test(`${machine} ${plane} quadrant ${shift}: major arcs, both full circles, repeats, work links and equivalent units`, () => {
    const outputs = [];
    for (const units of ["mm", "inch"]) {
      const f = geometryFixture(machine, plane, units, shift), r = translate(f.text, f.plan, {toolTable: f.toolTable});
      assert.match(r.gcode, /G43 H2\n/); assert.equal(r.report.toolTable.mappings[0].offset, 2);
      const actual = decodedGeometry(r.gcode), expected = decodedGeometry(f.reference);
      equivalent(actual, expected); outputs.push({r, actual});
      const base = geometryFixture(machine, plane, units), baseline = translate(base.text, base.plan), decodedBase = decodedGeometry(baseline.gcode);
      const shifted = structuredClone(decodedBase), [u,v] = frames[plane];
      for (const move of shifted.motions) if (move.frame === "work") for (const point of [move.end, move.center].filter(Boolean)) {
        if (point[u] !== null) point[u] += shift[0]; if (point[v] !== null) point[v] += shift[1];
      }
      equivalent(actual, shifted);
      assert.deepEqual(actual.motions.filter(m => m.frame === "machine"), decodedBase.motions.filter(m => m.frame === "machine"));
      const arcs = r.sourceMap.filter(e => e.command.type === "arc");
      assert.deepEqual(arcs.map(e => [e.command.clockwise, e.command.fullCircle]), [[true,false],[false,false],[true,true],[false,true]]);
      assert.deepEqual(arcs.slice(2).map(e => e.command.axes), [{},{}]);
      assert.equal(r.sourceMap.filter(e => e.kind === "linear" && e.phase === "toolpath" && e.action === "motion").length, 4);
    }
    equivalent(outputs[0].actual, outputs[1].actual);
    assert.notEqual(outputs[0].r.report.programFingerprint.value, outputs[1].r.report.programFingerprint.value);
  });
}
test("independent geometry comparisons catch scale, plane, centre, sense and frame mutations", () => {
  for (const [machine,plane] of configurations) {
    const f = geometryFixture(machine,plane,"mm",[-20,20]), r = translate(f.text,f.plan), expected = decodedGeometry(f.reference);
    const mutations = [code => code.replaceAll("G21", "G20"), code => code.replace(/^G2 /m, "G3 "),
      code => code.replace(/^G2 /m, `${plane === "XY" ? "G18" : "G17"}\nG2 `), code => code.replace(/([IJK])(-?\d+(?:\.\d+)?)/, (_,w,n) => w + -Number(n)),
      code => code.replace("G53 G0", "G0")];
    for (const mutate of mutations) { const bad = mutate(r.gcode); assert.notEqual(bad, r.gcode); assert.throws(() => equivalent(decodedGeometry(bad), expected), {name:"AssertionError"}); }
  }
});
test("tolerated joins before full circles retain actual commanded positions in both machines", () => {
  for (const [machine, plane] of configurations) {
    const [u,v] = frames[plane], start = [0,0,0], center = [0,0,0]; start[u] = 5;
    const p = new Program({machine, units:"mm"}), s = p.addSection({start, tool:{number:1,offset:1},workOffset:1,spindle:{mode:"rpm",speed:600,clockwise:true},coolant:"flood"});
    const feed = {mode:"perMinute",value:100};
    s.arc(start,center,true,feed,true,plane);
    // Source circle start differs within the reader's join tolerance. The next
    // exact linear endpoint must not be omitted on the basis of that source start.
    const near = [...start]; near[u] += 5e-10;
    s.paths[0].start = near; s.paths[0].end = near; s.paths[0].center = [...center]; s.paths[0].center[u] += 5e-10;
    // Insert the discontinuous source join directly, without an extra move.
    s.paths.push({kind:"linear",points:[near,[...near]],spindle:{mode:"rpm",speed:600,clockwise:true},coolant:"flood",feed});
    const text = p.toSTEP(), r = translate(text,simulationPlan(readProgram(text)));
    const arc = r.sourceMap.find(e => e.command.type === "arc"), last = r.sourceMap.filter(e => e.phase === "toolpath" && e.action === "motion").at(-1);
    assert.deepEqual(arc.command.axes, {}); assert.equal(arc.motion.end[u], start[u]);
    assert.equal(last.command.axes["xyz"[u]], near[u]); assert.equal(last.motion.start[u], start[u]); assert.equal(last.motion.end[u], near[u]);
    assert.equal(last.motion.end[v], 0);
  }
});
