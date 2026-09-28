"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const {translate} = require("../src/translate"), {readProgram} = require("../src/profile"), {template} = require("../src/plan");
const {linkExample, simulationPlan} = require("../scripts/example"), {Program} = require("../vendor/fusion360next-nc/next-nc");
test("reviewed work links preserve waypoint order, spindle state, offsets and exact next feed", () => {
  for (const units of ["mm", "inch"]) {
    const {text, plan} = linkExample(units), program = readProgram(text), out = translate(text, plan);
    assert.equal(program.report.operations[1].continuation.eligible, false);
    assert.equal(program.report.operations[1].connection.eligible, true);
    // A compatible state alone never invents a route in a generated template.
    assert.deepEqual(template(program).sections[1], {mode: "retract", retract: null, approach: null});
    const start = out.gcode.indexOf("(Section 2:"), end = out.gcode.lastIndexOf("M5 $0");
    const boundary = out.gcode.slice(start, end);
    assert.match(boundary, /G0 Z3\nG0 X14\nG0 Z2\nG0 X10\nG95 F0\.08\nG1 Z-2/);
    assert.doesNotMatch(boundary, /G53|M[3456789]|G43|G49|G9[267]/);
    assert.equal((out.gcode.match(/^M3 /gm) || []).length, 1);
    assert.equal((out.gcode.match(/^G53 /gm) || []).length, 4);
    assert.equal(out.report.execution.links, 1);
    assert.deepEqual(out.report.execution.transitions[1].start, [12, 0, 2]);
    assert.deepEqual(out.report.execution.transitions[1].end, [14, 0, 2]);
    const linkLines = out.sourceMap.map((s, i) => ({...s, line: out.gcode.split("\n")[i]})).filter(s => s.phase === "link");
    assert.deepEqual(linkLines.map(s => [s.waypoint, s.line]), [[1, "G0 Z3"], [2, "G0 X14"], [3, "G0 Z2"]]);
    // Unspecified Z inherits the known previous exit; single-axis link is valid.
    plan.sections[1].moves = [{x: 14}];
    assert.match(translate(text, plan).gcode, /\(Section 2: Second\)\nG0 X14\nG0 X10/);
  }
});
test("invalid links fail before output and older plan semantics stay unchanged", () => {
  const {text, plan} = linkExample();
  for (const [mutate, pattern] of [
    [p => {p.sections[0] = {mode: "link", moves: [{x: 12}, {z: 2}]};}, /First operation/],
    [p => {p.sections[1].moves = null;}, /explicit ordered/],
    [p => {p.sections[1].moves = [];}, /explicit ordered/],
    [p => {p.sections[1].moves = Array.from({length:1001}, () => ({x:14}));}, /explicit ordered/],
    [p => {p.sections[1].moves = [{x:14,z:2}];}, /one axis/],
    [p => {p.sections[1].moves = [{}];}, /one axis/],
    [p => {p.sections[1].moves = [{x:"14"}];}, /finite coordinate/],
    [p => {p.sections[1].moves = [{x:Infinity}];}, /finite coordinate/],
    [p => {p.sections[1].moves = [{x:NaN}];}, /finite coordinate/],
    [p => {p.sections[1].moves = [{x:1e9}];}, /finite coordinate/],
    [p => {p.sections[1].moves = [{x:14+1e-12}];}, /end exactly/],
    [p => {p.sections[1].moves = [{z:2}];}, /end exactly/],
    [p => {p.sections[1].moves = [{x:14,rawGcode:"M5"}];}, /unknown fields/],
    [p => {p.sections[1].approach = [{x:14},{z:2}];}, /unknown fields/],
    [p => {p.schema = "linuxcnc-next-nc/execution-plan/2";}, /unknown fields/],
    [p => {p.schema = "linuxcnc-next-nc/execution-plan/1";}, /unknown fields/]
  ]) {const bad=structuredClone(plan);mutate(bad);assert.throws(() => translate(text,bad),pattern);}
  for (const version of [1,2]) {
    const legacy=simulationPlan(readProgram(text));legacy.schema=`linuxcnc-next-nc/execution-plan/${version}`;
    if(version===1)legacy.sections.forEach(s=>delete s.mode);
    const out=translate(text,legacy);
    assert.equal(out.report.execution.links,0);assert.equal((out.gcode.match(/^G53 /gm)||[]).length,6);
    assert.equal((out.gcode.match(/^M3 /gm)||[]).length,2);
  }
});
test("links cannot bypass tool, offset, spindle or coolant changes", () => {
  const spec={tool:{number:1,offset:1},workOffset:1,start:[12,0,2],spindle:{mode:"css",speed:80000,maximumRPM:1800,clockwise:true},coolant:"off"};
  for(const change of [
    {tool:{number:2,offset:1}},{tool:{number:1,offset:2}},{workOffset:2},{coolant:"flood"},
    {spindle:{...spec.spindle,speed:81000}},{spindle:{...spec.spindle,maximumRPM:1700}},
    {spindle:{...spec.spindle,clockwise:false}},{spindle:{mode:"rpm",speed:1000,clockwise:true}}
  ]) {
    const p=new Program({units:"mm"});p.addSection(spec).dwell(.1);p.addSection({...spec,start:[14,0,2],...change}).dwell(.1);
    const text=p.toSTEP(),program=readProgram(text),plan=simulationPlan(program);plan.sections[1]={mode:"link",moves:[{x:14}]};
    assert.equal(program.report.operations[1].connection.eligible,false);
    assert.throws(()=>translate(text,plan),e=>e.code==="LINK_STATE");
  }
});
