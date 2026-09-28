"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const {millingExample, simulationPlan, example} = require("../scripts/example");
const {translate} = require("../src/translate"), {readProgram} = require("../src/profile"), {template} = require("../src/plan");
const {Program} = require("../vendor/fusion360next-nc/next-nc");
test("XYZ mill in both units preserves Y, plane changes, independent H offsets and both arc senses", () => {
  for (const units of ["mm", "inch"]) {
    const {text, plan} = millingExample(units), program = readProgram(text), out = translate(text, plan, {toolTable: "T1 P1\nT2 P2\nT3 P3\n"});
    assert.equal(program.report.machine, "mill"); assert.equal(program.model.machine, "mill"); assert.equal(program.model.units, units);
    assert.equal(template(program).machine, "mill"); assert.equal(plan.schema, "linuxcnc-next-nc/execution-plan/4");
    assert.match(out.gcode, units === "mm" ? /G21 G17 G8 G90/ : /G20 G17 G8 G90/);
    assert.match(out.gcode, /T2 M6[\s\S]*G55\nG43 H3/); assert.match(out.gcode, /G1 X4 Y5 Z0/);
    const arcBlocks = out.gcode.split("\n").filter(line => /^G[23] /.test(line));
    assert.deepEqual(arcBlocks, ["G3 X1 Y2 I-1 J0", "G2 X2 Y1 I0 J-1", "G2 I-1 J0 P1", "G3 X1 Z-2 I-1 K0", "G2 X2 Z-1 I0 K1", "G2 I-1 K0 P1", "G3 Y0 Z0 J-1 K0", "G2 Y1 Z-1 J0 K-1", "G2 J-1 K0 P1"]);
    assert.equal(out.gcode.match(/^G18$/gm).length, 1); assert.equal(out.gcode.match(/^G19$/gm).length, 1);
    const arcs = out.sourceMap.filter(x => x.motion?.kind === "arc");
    assert.deepEqual(arcs.map(x => x.motion.plane), ["XY", "XY", "XY", "XZ", "XZ", "XZ", "YZ", "YZ", "YZ"]);
    assert.deepEqual(arcs[8].motion.end, [2, 1, -1]); assert.deepEqual(arcs[8].motion.centerOffset, [0, -1, 0]);
    assert.deepEqual(out.report.bounds, {min: [0, -1, -2], max: [4, 5, 5]});
    assert.equal(out.report.toolTable.status, "passed");
  }
});
test("milling plan rejects missing Y, wrong machine, old schema, and incorrect Y entry before emission", () => {
  const {text, plan} = millingExample();
  for (const mutate of [p => {delete p.machine;}, p => {p.machine = "lathe";}, p => {p.schema = "linuxcnc-next-nc/execution-plan/3";}, p => {p.sections[0].retract = [{z: 10}, {x: 0}];}, p => {p.sections[0].approach[2].y = 0;}, p => {p.end = [{z: 10}, {x: 0}];}]) {
    const bad = structuredClone(plan); mutate(bad); assert.throws(() => translate(text, bad));
  }
  const lathe = example(); lathe.plan.schema = "linuxcnc-next-nc/execution-plan/4"; lathe.plan.machine = "mill";
  assert.throws(() => translate(lathe.text, lathe.plan), e => e.code === "PLAN_MACHINE");
});
test("milling continuations and reviewed links preserve exact XYZ endpoints", () => {
  const p = new Program({machine: "mill", units: "mm"}), spec = {tool: {number: 1, offset: 1}, workOffset: 1, start: [1, 2, 3], spindle: {mode: "rpm", speed: 800, clockwise: true}, coolant: "off"};
  p.addSection(spec).linear([2, 3, 4], {mode: "perMinute", value: 40});
  p.addSection({...spec, start: [2, 3, 4]}).linear([2, 5, 4], {mode: "perMinute", value: 30});
  p.addSection({...spec, start: [3, 6, 4]}).linear([3, 6, 0], {mode: "perMinute", value: 20});
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text)); plan.sections[2] = {mode: "link", moves: [{y: 6}, {x: 3}]};
  const out = translate(text, plan); assert.equal(out.report.execution.continuations, 1); assert.equal(out.report.execution.links, 1);
  assert.match(out.gcode, /G0 Y6\nG0 X3\nG94 F20\nG1 Z0/);
  plan.sections[2].moves = [{x: 3}]; assert.throws(() => translate(text, plan), e => e.code === "LINK_ENDPOINT");
});
