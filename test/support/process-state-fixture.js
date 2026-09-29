"use strict";
const {Program} = require("../../vendor/fusion360next-nc/next-nc");
const {readProgram} = require("../../src/profile"), {simulationPlan} = require("../../scripts/example");
function processStateFixture(machine, units, coolant = "off") {
  const mill = machine === "mill", p = new Program({machine, units, name: "Process state simulation only"});
  const spec = {tool: {number: 1, offset: 3}, workOffset: 1, start: [4,0,2], spindle: {mode: "rpm", speed: 500, clockwise: true}, coolant};
  const first = p.addSection({name: "Equal values and dwell changes", ...spec});
  first.setSpindle({mode: "rpm", speed: 600, clockwise: true});
  first.linear([3,0,1], {mode: "perMinute", value: 0.2});
  first.setSpindle({mode: "rpm", speed: 600, clockwise: false}); first.setCoolant("mist"); first.dwell(0.25);
  first.linear([2,0,0], {mode: "perRevolution", value: 0.2}); first.rapid([4,0,2]);
  first.linear([3,0,1], {mode: "perRevolution", value: 0.2});
  if (!mill) {
    const css = {mode: "css", speed: 600 * (units === "mm" ? 1000 : 12), maximumRPM: 1800, clockwise: false};
    first.setSpindle(css); first.dwell(0.5);
    first.setSpindle({...css, maximumRPM: 1900}); first.dwell(0.75);
    first.setSpindle({mode: "rpm", speed: 600, clockwise: false}); first.dwell(1);
  }
  const next = {...spec, spindle: {mode: "rpm", speed: 600, clockwise: true}};
  p.addSection({name: "Later M6", ...next, tool: {number: 2, offset: 1}}).linear([3,0,1], {mode: "perMinute", value: 0.2});
  p.addSection({name: "Same tool, different H and WCS", ...next, tool: {number: 2, offset: 3}, workOffset: 2}).linear([3,0,1], {mode: "perMinute", value: 0.2});
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text));
  plan.tools = {"1:3": {tool: 1, offset: 3}, "2:1": {tool: 2, offset: 1}, "2:3": {tool: 2, offset: 3}};
  plan.workOffsets[2] = "G55";
  // Independent hand-written instruction oracle. No production output helpers.
  const init = `${units === "mm" ? "G21" : "G20"} ${mill ? "G17" : "G18"} G8 G90 G91.1 G40 G80 G94 G61`;
  const stop = ["M5 $0", "M9", "G97 S0 $0"], retract = mill ? ["G53 G0 Z20", "G53 G0 X0", "G53 G0 Y0"] : ["G53 G0 X25", "G53 G0 Z10"];
  const approach = mill ? ["G0 Z10", "G0 X4", "G0 Y0", "G0 Z2"] : ["G0 Z2", "G0 X4"];
  const on = coolant === "off" ? [] : ["M9", coolant === "flood" ? "M8" : "M7"];
  const reference = [init, "G92.1", ...stop, ...retract, "G49", "T1 M6", init, ...stop, "G92.1", "G54", "G43 H3", ...approach,
    "G97 S500 $0", "M3 $0", ...on, "G97 S600 $0", "M3 $0", "G94 F0.2", "G1 X3 Z1",
    "M5 $0", "G97 S600 $0", "M4 $0", ...(coolant === "mist" ? [] : ["M9", "M7"]), "G4 P0.25",
    "G95 F0.2", "G1 X2 Z0", "G0 X4 Z2", "G1 X3 Z1"];
  if (!mill) reference.push("G96 D1800 S600 $0", "M4 $0", "G4 P0.5", "G96 D1900 S600 $0", "M4 $0", "G4 P0.75", "G97 S600 $0", "M4 $0", "G4 P1");
  reference.push(...stop, ...retract, "G49", "T2 M6", init, ...stop, "G92.1", "G54", "G43 H1", ...approach,
    "G97 S600 $0", "M3 $0", ...on, "G94 F0.2", "G1 X3 Z1",
    ...stop, ...retract, init, "G92.1", "G55", "G43 H3", ...approach, "G97 S600 $0", "M3 $0", ...on, "G94 F0.2", "G1 X3 Z1",
    ...stop, ...retract, "G49", "G94", "M2", "");
  return {text, plan, reference: reference.join("\n"),
    toolTable: `T1 P1 X1 Y${mill ? 2 : 0} Z3\nT2 P2 X7 Y${mill ? 8 : 0} Z9\nT3 P3 X4 Y${mill ? 5 : 0} Z6\n`,
    parameters: `5220 1\n5221 10\n5222 ${mill ? 20 : 0}\n5223 30\n5241 40\n5242 ${mill ? 50 : 0}\n5243 60\n`};
}
module.exports = {processStateFixture};
