"use strict";
const fs = require("node:fs"), path = require("node:path");
const {Program} = require("../vendor/fusion360next-nc/next-nc"), {readProgram} = require("../src/profile");
const {template, machineOf} = require("../src/plan"), {translate} = require("../src/translate");
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
  const mill = machineOf(program) === "mill";
  for (const key of Object.keys(plan.tools)) plan.tools[key] = {tool: 1, offset: 1};
  for (const key of Object.keys(plan.workOffsets)) plan.workOffsets[key] = "G54";
  plan.sections = program.model.sections.map((s, i) => plan.sections[i].mode === "continue" ? plan.sections[i] : {mode: "retract", retract: mill ? [{z: 20}, {x: 0}, {y: 0}] : [{x: 25}, {z: 10}], approach: mill ? [{z: 10}, {x: s.start[0]}, {y: s.start[1]}, {z: s.start[2]}] : [{z: s.start[2]}, {x: s.start[0]}]});
  plan.end = mill ? [{z: 20}, {x: 0}, {y: 0}] : [{x: 25}, {z: 10}]; return plan;
}
function millingExample(units = "mm") {
  const p = new Program({name: "Synthetic XYZ simulation only", machine: "mill", units, timestamp: "2026-09-28T00:00:00Z"});
  const spec = {tool: {number: 1, offset: 1}, workOffset: 1, start: [2, 1, 5], spindle: {mode: "rpm", speed: 1200, clockwise: true}, coolant: "flood"};
  const s = p.addSection({name: "Three-plane arcs", ...spec}), feed = {mode: "perMinute", value: 100};
  s.rapid([2, 1, 0]); s.linear([2, 1, -1], feed);
  for (const [plane, end, center] of [["XY", [1, 2, -1], [1, 1, -1]], ["XZ", [1, 1, -2], [1, 1, -1]], ["YZ", [2, 0, 0], [2, 0, -1]]]) {
    s.arc(end, center, false, feed, false, plane);
    s.arc([2, 1, -1], center, true, feed, false, plane);
    s.arc([2, 1, -1], center, true, feed, true, plane);
  }
  s.dwell(0.2);
  p.addSection({name: "Second tool XYZ", ...spec, tool: {number: 2, offset: 3}, workOffset: 2, start: [3, 4, 5]}).linear([4, 5, 0], {mode: "perMinute", value: 80});
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text));
  plan.tools["2:3"] = {tool: 2, offset: 3}; plan.workOffsets[2] = "G55";
  return {text, plan};
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
function linkExample(units = "mm") {
  const p = new Program({name: "Synthetic reviewed connection", units, timestamp: "2026-09-28T00:00:00Z"});
  const spec = {tool: {number: 1, offset: 1, description: "Simulation tool"}, workOffset: 1,
    spindle: {mode: "css", speed: units === "mm" ? 80000 : 1200, maximumRPM: 1800, clockwise: true}, coolant: "off"};
  for (const [name, x, feed, depth] of [["First", 12, 0.18, -1], ["Second", 14, 0.08, -2]]) {
    const s = p.addSection({name, start: [x, 0, 2], ...spec});
    s.rapid([10, 0, 2]); s.linear([10, 0, depth], {mode: "perRevolution", value: feed});
    s.rapid([x, 0, depth]); s.rapid([x, 0, 2]);
  }
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text));
  // Synthetic simulation path: deliberately preserve the explicit detour/order.
  plan.sections[1] = {mode: "link", moves: [{z: 3}, {x: 14}, {z: 2}]};
  return {text, plan};
}
if (require.main === module) {
  const {text, plan} = example(), dir = path.join(__dirname, "../examples");
  fs.writeFileSync(path.join(dir, "synthetic.stpnc"), text);
  fs.writeFileSync(path.join(dir, "simulation-plan.json"), JSON.stringify(plan, null, 2) + "\n");
  fs.writeFileSync(path.join(dir, "synthetic.ngc"), translate(text, plan).gcode);
  const mill = millingExample();
  fs.writeFileSync(path.join(dir, "synthetic-mill.stpnc"), mill.text);
  fs.writeFileSync(path.join(dir, "simulation-mill-plan.json"), JSON.stringify(mill.plan, null, 2) + "\n");
  fs.writeFileSync(path.join(dir, "synthetic-mill.ngc"), translate(mill.text, mill.plan).gcode);
}
module.exports = {example, simulationPlan, continuationExample, linkExample, millingExample};
