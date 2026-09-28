"use strict";
const {Program} = require("../../vendor/fusion360next-nc/next-nc");
const {readProgram} = require("../../src/profile"), {simulationPlan} = require("../../scripts/example");
function semanticFixture(machine, units) {
  const mill = machine === "mill", p = new Program({machine, units, name: "Independent semantic fixture"});
  const spindle = mill ? {mode: "rpm", speed: 600, clockwise: true} : {mode: "css", speed: units === "mm" ? 80000 : 1200, maximumRPM: 1800, clockwise: true};
  const spec = {tool: {number: 1, offset: 3}, workOffset: 1, spindle, coolant: "flood"};
  const feed = {mode: "perMinute", value: 100};
  const firstEnd = mill ? [4, -2, 0] : [2, 0, 0], secondEnd = mill ? [2, -2, -2] : [4, 0, 2];
  p.addSection({name: "First", ...spec, start: [4, 0, 2]}).arc(firstEnd, mill ? [4, -2, 2] : [2, 0, 2], false, feed, false, mill ? "YZ" : "XZ");
  p.addSection({name: "Continue", ...spec, start: firstEnd}).arc(secondEnd, mill ? [2, -2, 0] : [2, 0, 2], !mill, {mode: "perMinute", value: 80}, false, "XZ");
  const s = p.addSection({name: "Link and reverse", ...spec, start: mill ? [6, 2, 2] : [6, 0, 2]});
  s.arc(mill ? [4, 4, 2] : [4, 0, 0], mill ? [4, 2, 2] : [4, 0, 2], true, feed, false, mill ? "XY" : "XZ");
  s.dwell(0.25); s.rapid(mill ? [4, 4, 3] : [4, 0, 3]);
  s.setSpindle({mode: "rpm", speed: 700, clockwise: false}); s.setCoolant("mist");
  s.linear(mill ? [3, 3, 1] : [3, 0, 1], {mode: "perRevolution", value: 0.2});
  p.addSection({name: "Retract and new tool", ...spec, tool: {number: 2, offset: 1}, workOffset: 2,
    start: mill ? [8, 1, 4] : [8, 0, 4], spindle: {mode: "rpm", speed: 900, clockwise: true}}).linear(mill ? [7, 2, 0] : [7, 0, 0], {mode: "perMinute", value: 60});
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text));
  plan.tools = {"1:3": {tool: 1, offset: 3}, "2:1": {tool: 2, offset: 1}}; plan.workOffsets[2] = "G55";
  plan.sections[1] = {mode: "continue"}; plan.sections[2] = {mode: "link", moves: mill ? [{z: 2}, {x: 6}, {y: 2}] : [{z: 2}, {x: 6}]};
  const toolTable = `T1 P1 X1 Y${mill ? 2 : 0} Z3\nT2 P2 X7 Y${mill ? 8 : 0} Z9\nT3 P3 X4 Y${mill ? 5 : 0} Z6\n`;
  const parameters = `5220 1\n5221 10\n5222 ${mill ? 20 : 0}\n5223 30\n5241 40\n5242 ${mill ? 50 : 0}\n5243 60\n`;
  return {text, plan, toolTable, parameters};
}
// Hand-authored executable oracle, independent of the writer, reader, plan and
// translator. Explicit redundant axes are intentional. Coordinates are synthetic.
function referenceGcode(machine, units) {
  const mill = machine === "mill";
  const initialize = `${units === "mm" ? "G21" : "G20"} ${mill ? "G17" : "G18"} G8 G90 G91.1 G40 G80 G94 G61`;
  const stop = "M5 $0\nM9\nG97 S0 $0";
  const retract = mill ? "G53 G0 Z20\nG53 G0 X0\nG53 G0 Y0" : "G53 G0 X25\nG53 G0 Z10";
  return [initialize, "G92.1", stop, retract, "G49", "T1 M6", initialize, "G92.1", "G54", "G43 H3",
    mill ? "G0 Z10\nG0 X4\nG0 Y0\nG0 Z2" : "G0 Z2\nG0 X4",
    mill ? "G97 S600 $0" : `G96 D1800 S${units === "mm" ? 80 : 100} $0`, "M3 $0", "M9", "M8", "G94 F100",
    mill ? "G19\nG3 Y-2 Z0 J-2 K0" : "G3 X2 Z0 I-2 K0",
    "G94 F80", mill ? "G18\nG3 X2 Z-2 I-2 K0" : "G2 X4 Z2 I0 K2",
    mill ? "G0 Z2\nG0 X6\nG0 Y2" : "G0 Z2\nG0 X6", "G94 F100",
    mill ? "G17\nG2 X4 Y4 I-2 J0" : "G2 X4 Z0 I-2 K0", "G4 P0.25", "G0 Z3",
    "M5 $0", "G97 S700 $0", "M4 $0", "M9", "M7", "G95 F0.2", mill ? "G1 X3 Y3 Z1" : "G1 X3 Z1",
    stop, retract, "G49", "T2 M6", initialize, "G92.1", "G55", "G43 H1",
    mill ? "G0 Z10\nG0 X8\nG0 Y1\nG0 Z4" : "G0 Z4\nG0 X8",
    "G97 S900 $0", "M3 $0", "M9", "M8", "G94 F60", mill ? "G1 X7 Y2 Z0" : "G1 X7 Z0",
    stop, retract, "G49", "G94", "M2", ""].join("\n");
}
module.exports = {semanticFixture, referenceGcode};
