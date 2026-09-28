"use strict";
const fs = require("node:fs"), path = require("node:path");
const {Program} = require("../vendor/fusion360next-nc/next-nc"), {readProgram} = require("../src/profile");
const {template} = require("../src/plan"), {translate} = require("../src/translate");
function example(units = "mm") {
  const p = new Program({name: "Synthetic simulation only", units, timestamp: "2026-09-28T00:00:00Z"});
  const s = p.addSection({name: "Synthetic turning", tool: {number: 1, offset: 1, description: "Simulation tool"}, workOffset: 1,
    start: [12, 0, 2], spindle: {mode: "css", speed: units === "mm" ? 80000 : 1200, maximumRPM: 1800, clockwise: true}, coolant: "off"});
  s.rapid([10, 0, 2]); s.linear([10, 0, 0], {mode: "perRevolution", value: 0.1});
  s.arc([8, 0, -2], [8, 0, 0], false, {mode: "perRevolution", value: 0.1}, false);
  s.arc([10, 0, 0], [8, 0, 0], true, {mode: "perRevolution", value: 0.1}, false);
  s.dwell(0.25); s.setSpindle({mode: "rpm", speed: 700, clockwise: false}); s.setCoolant("mist");
  s.arc([10, 0, 0], [8, 0, 0], true, {mode: "perMinute", value: 100}, true); s.rapid([12, 0, 2]);
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text)); return {text, plan};
}
function simulationPlan(program) {
  const plan = template(program);
  for (const key of Object.keys(plan.tools)) plan.tools[key] = {tool: 1, offset: 1};
  for (const key of Object.keys(plan.workOffsets)) plan.workOffsets[key] = "G54";
  plan.sections = program.model.sections.map((s, i) => plan.sections[i].mode === "continue" ? plan.sections[i] : {mode: "retract", retract: [{x: 25}, {z: 10}], approach: [{z: s.start[2]}, {x: s.start[0]}]});
  plan.end = [{x: 25}, {z: 10}]; return plan;
}
function continuationExample(units = "mm") {
  const p = new Program({name: "Synthetic continuous operations", units, timestamp: "2026-09-28T00:00:00Z"});
  const spec = {tool: {number: 1, offset: 1, description: "Simulation tool"}, workOffset: 1, start: [12, 0, 2],
    spindle: {mode: "css", speed: units === "mm" ? 80000 : 1200, maximumRPM: 1800, clockwise: true}, coolant: "off"};
  for (const [name, feed, depth] of [["First", 0.18, -1], ["Second", 0.08, -2]]) {
    const s = p.addSection({name, ...spec});
    s.rapid([10, 0, 2]); s.linear([10, 0, depth], {mode: "perRevolution", value: feed});
    s.rapid([12, 0, depth]); s.rapid([12, 0, 2]);
  }
  const text = p.toSTEP(); return {text, plan: simulationPlan(readProgram(text))};
}
if (require.main === module) {
  const {text, plan} = example(), dir = path.join(__dirname, "../examples");
  fs.writeFileSync(path.join(dir, "synthetic.stpnc"), text);
  fs.writeFileSync(path.join(dir, "simulation-plan.json"), JSON.stringify(plan, null, 2) + "\n");
  fs.writeFileSync(path.join(dir, "synthetic.ngc"), translate(text, plan).gcode);
}
module.exports = {example, simulationPlan, continuationExample};
