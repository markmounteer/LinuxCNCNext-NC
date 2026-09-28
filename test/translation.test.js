"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const {translate, decimal} = require("../src/translate"), {readProgram} = require("../src/profile"), {template} = require("../src/plan");
const {example, simulationPlan, continuationExample} = require("../scripts/example");
const {Program} = require("../vendor/fusion360next-nc/next-nc");
test("mm/inch CSS scaling, radius mode, plane, arcs, feeds, reversal and dwell", () => {
  for (const units of ["mm", "inch"]) {
    const {text, plan} = example(units), out = translate(text, plan);
    assert.match(out.gcode, units === "mm" ? /G21 G18 G8 G90 G91\.1/ : /G20 G18 G8 G90 G91\.1/);
    assert.match(out.gcode, units === "mm" ? /G96 D1800 S80 \$0/ : /G96 D1800 S100 \$0/);
    assert.match(out.gcode, /G95 F0\.1/); assert.match(out.gcode, /G94 F100/);
    assert.match(out.gcode, /G3 X8 Z-2 I-2 K0/); assert.match(out.gcode, /G2 X10 Z0 I0 K2/);
    assert.match(out.gcode, /G2 I-2 K0 P1/); assert.match(out.gcode, /G4 P0\.25/);
    assert.match(out.gcode, /M5 \$0\nG97 S700 \$0\nM4 \$0/); assert.match(out.gcode, /M9\nM7/);
    assert.equal(out.sourceMap.length, out.report.gcodeLines); assert.ok(out.gcode.endsWith("M2\n"));
    assert.equal(out.gcode.match(/^G[23] /gm).length, out.report.arcs);
  }
});
test("all vertices and repeated curves survive with exact feeds and scoped machine transitions", () => {
  const p = new Program({units: "mm"}), spec = {tool: {number: 7, offset: 4}, workOffset: 0,
    start: [2, 0, 0], spindle: {mode: "rpm", speed: 1200, clockwise: true}, coolant: "flood"};
  for (let i = 0; i < 2; ++i) {
    const s = p.addSection({name: "Repeated", ...spec});
    s.linear([3, 0, 0], {mode: "perMinute", value: 0.123456789012345});
    s.linear([4, 0, -1], {mode: "perMinute", value: 0.123456789012345});
  }
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text)); plan.tools["7:4"] = {tool: 9, offset: 8}; plan.workOffsets[0] = "G59.3";
  const out = translate(text, plan);
  assert.equal(out.gcode.match(/^G1 X3$/gm).length, 2); assert.equal(out.gcode.match(/G1 X4 Z-1/g).length, 2);
  assert.equal(out.gcode.match(/T9 M6/g).length, 1); assert.equal(out.gcode.match(/G43 H8/g).length, 2);
  assert.equal(out.gcode.match(/G59\.3/g).length, 2); assert.match(out.gcode, /F0\.123456789012345/);
  assert.match(out.gcode, /G53 G0 X25\nG53 G0 Z10/); assert.ok(!/G41|G42|G10 |G28|G30/.test(out.gcode));
});

test("continuous operations retain process state, exact geometry and the changed finishing feed", () => {
  for (const units of ["mm", "inch"]) {
    const {text, plan} = continuationExample(units), program = readProgram(text);
    assert.equal(program.report.operations[0].continuation.eligible, false);
    assert.equal(program.report.operations[1].continuation.eligible, true);
    assert.deepEqual(plan.sections[1], {mode: "continue"});
    const out = translate(text, plan), boundary = out.gcode.indexOf("(Section 2:");
    const body = out.gcode.slice(boundary, out.gcode.lastIndexOf("M5 $0"));
    assert.doesNotMatch(body, /M[356789]|G53|G43|G49|G9[267]/);
    assert.match(body, /G95 F0\.08\nG1 Z-2/);
    assert.equal((out.gcode.match(/^M3 /gm) || []).length, 1);
    assert.equal((out.gcode.match(/^G43 /gm) || []).length, 1);
    assert.equal((out.gcode.match(/^G53 /gm) || []).length, 4);
    assert.equal(out.report.execution.continuations, 1);
    assert.equal(out.report.execution.unchangedAxisWordsOmitted, 8);
    assert.equal(out.report.execution.coordinatesRounded, false);
    // Independently decode every emitted body vertex through modal coordinates.
    for (let i = 0; i < program.model.sections.length; i++) {
      let position = program.model.sections[i].start.slice(); const actual = [];
      out.gcode.trim().split("\n").forEach((line, n) => {
        if (out.sourceMap[n].section !== i + 1 || !out.sourceMap[n].path || !/^G[01] /.test(line)) return;
        for (const [, axis, value] of line.matchAll(/([XZ])(-?[\d.]+)/g)) position[axis === "X" ? 0 : 2] = Number(value);
        actual.push(position.slice());
      });
      const expected = program.model.sections[i].paths.flatMap(p => p.points.slice(1));
      assert.deepEqual(actual, expected);
    }
  }
});

test("existing reviewed retracts remain honored and invalid continuations fail closed", () => {
  const {text, plan} = continuationExample();
  const legacy = structuredClone(plan); legacy.schema = "linuxcnc-next-nc/execution-plan/1";
  legacy.sections = legacy.sections.map(() => ({retract: [{x: 25}, {z: 10}], approach: [{z: 2}, {x: 12}]}));
  const legacyOut = translate(text, legacy);
  assert.equal(legacyOut.report.execution.continuations, 0);
  assert.equal((legacyOut.gcode.match(/^G53 /gm) || []).length, 6);
  assert.equal((legacyOut.gcode.match(/^M3 /gm) || []).length, 2);
  const explicit = structuredClone(plan); explicit.sections[1] = {...legacy.sections[1], mode: "retract"};
  assert.equal(translate(text, explicit).report.execution.continuations, 0);
  const first = structuredClone(plan); first.sections[0] = {mode: "continue"};
  assert.throws(() => translate(text, first), /First operation/);
  const both = structuredClone(plan); both.sections[1].retract = [{x: 25}, {z: 10}];
  assert.throws(() => translate(text, both), /unknown fields/);
  const oldContinue = structuredClone(legacy); oldContinue.sections[1] = {mode: "continue"};
  assert.throws(() => translate(text, oldContinue), /unknown fields/);
  for (const [field, value, reason] of [
    ["start", [12 + 1e-12, 0, 2], /exit and next entry/],
    ["tool", {number: 2, offset: 1}, /tool or tool offset/],
    ["tool", {number: 1, offset: 2}, /tool or tool offset/],
    ["workOffset", 2, /work offset differs/],
    ["spindle", {mode: "css", speed: 80000, maximumRPM: 1700, clockwise: true}, /Spindle/],
    ["spindle", {mode: "css", speed: 81000, maximumRPM: 1800, clockwise: true}, /Spindle/],
    ["spindle", {mode: "css", speed: 80000, maximumRPM: 1800, clockwise: false}, /Spindle/],
    ["spindle", {mode: "rpm", speed: 1000, clockwise: true}, /Spindle/],
    ["coolant", "flood", /Coolant/]
  ]) {
    const p = new Program({units: "mm"});
    const spec = {tool: {number: 1, offset: 1}, workOffset: 1, start: [12, 0, 2],
      spindle: {mode: "css", speed: 80000, maximumRPM: 1800, clockwise: true}, coolant: "off"};
    p.addSection(spec).dwell(0.1); p.addSection({...spec, [field]: value}).dwell(0.1);
    const changed = p.toSTEP(), parsed = readProgram(changed), badPlan = simulationPlan(parsed);
    assert.equal(parsed.report.operations[1].continuation.eligible, false);
    badPlan.sections[1] = {mode: "continue"};
    assert.throws(() => translate(changed, badPlan), reason);
  }
});

test("continuation examines final arc/dwell state, and axis omission preserves reversals and repeats", () => {
  const p = new Program({units: "mm"}), spec = {tool: {number: 1, offset: 1}, workOffset: 1,
    start: [10, 0, 0], spindle: {mode: "rpm", speed: 600, clockwise: true}, coolant: "off"};
  const s = p.addSection(spec);
  s.arc([10, 0, 0], [8, 0, 0], true, {mode: "perMinute", value: 100}, true);
  s.setSpindle({mode: "rpm", speed: 700, clockwise: false}); s.setCoolant("mist"); s.dwell(0.2);
  const s2 = p.addSection({...spec, spindle: {mode: "rpm", speed: 700, clockwise: false}, coolant: "mist"});
  s2.linear([9, 0, 0], {mode: "perMinute", value: 80}); s2.linear([10, 0, 0], {mode: "perMinute", value: 80});
  const text = p.toSTEP(), program = readProgram(text), plan = simulationPlan(program), out = translate(text, plan);
  assert.equal(plan.sections[1].mode, "continue"); assert.match(out.gcode, /G2 I-2 K0 P1/);
  assert.match(out.gcode, /G1 X9\nG1 X10/); assert.equal(out.report.execution.continuations, 1);
});

test("axis omission uses commanded coordinates across reader-tolerated curve joins", () => {
  const p = new Program({units: "mm"});
  const s = p.addSection({tool: {number: 1, offset: 1}, workOffset: 1, start: [6, 0, 0],
    spindle: {mode: "rpm", speed: 600, clockwise: true}, coolant: "off"});
  s.rapid([5, 0, 0]); s.linear([5.0000000005, 0, -1], {mode: "perMinute", value: 100});
  let text = p.toSTEP();
  const curves = [...text.matchAll(/#\d+=POLYLINE\('',\(#\d+,/g)];
  const id = Math.max(...[...text.matchAll(/^#(\d+)=/gm)].map(m => Number(m[1]))) + 1;
  text = text.replace(curves[1][0], curves[1][0].replace(/\(#\d+,/, `(#${id},`));
  text = text.replace("ENDSEC;\nEND-ISO-10303-21;", `#${id}=CARTESIAN_POINT('',(5.0000000005,0.,0.));\nENDSEC;\nEND-ISO-10303-21;`);
  const program = readProgram(text), out = translate(text, simulationPlan(program));
  assert.match(out.gcode, /G0 X5\nG94 F100\nG1 X5\.0000000005 Z-1/);
});
test("unknown required semantics, incomplete documents and ambiguous dwell are rejected", () => {
  const {text, plan} = example();
  for (const corrupted of [text.slice(0, -8), text.replaceAll("'priority'", "'unsupported priority'"),
    text.replace("MACHINING_TOOLPATH(", "MACHINING_NC_FUNCTION("), text.replace("#1=", "#999999=")]) assert.throws(() => translate(corrupted, plan));
  assert.throws(() => readProgram(text.replace("'tool center point'", "'other reference'")), /feed reference/);
  assert.throws(() => readProgram(text.replace("'mist'", "'through tool'")), /Through-tool/);
  assert.throws(() => readProgram(text.replace("ACTION_PROPERTY('dwell',", "ACTION_PROPERTY('basic curve',")), /motion\/dwell/);
});
test("plan templates fail closed; changed program, mappings, units and entries are diagnosed", () => {
  const {text, plan} = example();
  assert.throws(() => translate(text, template(readProgram(text))), /positive integers/);
  for (const [mutate, pattern] of [
    [p => { p.programFingerprint = "a".repeat(64); }, /different decoded program/],
    [p => { p.units = "inch"; }, /same units/], [p => { delete p.tools["1:1"]; }, /Map Fusion/],
    [p => { p.tools["1:1"].offset = 0; }, /positive integers/], [p => { p.workOffsets[1] = "G54\nM3"; }, /Map Fusion work offset/],
    [p => { p.sections[0].retract = null; }, /explicit ordered/], [p => { p.sections[0].approach[1].x = 11; }, /end exactly/],
    [p => { p.end = [{x: 1, z: 1}]; }, /explicit ordered/], [p => { p.sections[0].rawGcode = "G1 X0"; }, /unknown fields/]
  ]) { const changed = structuredClone(plan); mutate(changed); assert.throws(() => translate(text, changed), pattern); }
});
test("metadata cannot inject active G-code and tiny values never use exponent notation", () => {
  const p = new Program({units: "mm", name: ")\nM3\n(MSG,evil)"});
  p.addSection({name: ")\nG0 X999", tool: {number: 1, offset: 1}, workOffset: 1, start: [1, 0, 0],
    spindle: {mode: "rpm", speed: 600, clockwise: true}, coolant: "off"}).linear([1e-12, 0, -1e-7], {value: 0.1, mode: "perMinute"});
  const text = p.toSTEP(), out = translate(text, simulationPlan(readProgram(text)));
  assert.ok(!out.gcode.split("\n").some(line => line === "M3" || line === "G0 X999"));
  assert.match(out.gcode, /G1 X0\.000000000001 Z-0\.0000001/);
  for (const n of [1e-12, -1e-10, 1e12, 0.123456789012345, -0]) { const value = decimal(n); assert.ok(!/[eE]/.test(value)); assert.equal(Number(value), n === 0 ? 0 : n); }
  assert.throws(() => decimal(NaN)); assert.throws(() => decimal(1e-200));
  for (const name of ["ABORT,stop", "LOGOPEN,/tmp/should-not-open", "DEBUG,#<_x>", "MSG,unexpected"]) {
    p.name = name; const active = p.toSTEP(), result = translate(active, simulationPlan(readProgram(active)));
    assert.ok(!/^\((?:ABORT|LOGOPEN|DEBUG|MSG),/m.test(result.gcode)); assert.match(result.gcode, /\(Program: /);
  }
});
