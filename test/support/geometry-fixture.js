"use strict";
const {Program} = require("../../vendor/fusion360next-nc/next-nc");
const {readProgram} = require("../../src/profile"), {simulationPlan} = require("../../scripts/example");
// Right-handed in-plane coordinates, specified independently of production:
// +Z: X,Y; +Y: Z,X; +X: Y,Z. No rotary tool-axis motion is involved.
const frames = {XY: [0, 1], XZ: [2, 0], YZ: [1, 2]};
const configurations = [["lathe", "XZ"], ["mill", "XY"], ["mill", "XZ"], ["mill", "YZ"]];
function geometryFixture(machine, plane, units, shift = [0, 0]) {
  const scale = units === "mm" ? 25.4 : 1, [u, v] = frames[plane], fixed = [0,1,2].find(a => a !== u && a !== v);
  function point(a, b) { const p = [0,0,0]; p[u] = (a + shift[0]) * scale; p[v] = (b + shift[1]) * scale; p[fixed] = machine === "mill" ? 2 * scale : 0; return p; }
  const a = point(4,3), b = point(-3,4), c = point(0,0), repeat = point(5,3), link = point(6,3), end = point(7,3);
  const spindle = machine === "lathe" ? {mode: "css", speed: units === "mm" ? 30480 : 1200, clockwise: true, maximumRPM: 1800} : {mode: "rpm", speed: 600, clockwise: true};
  const p = new Program({machine, units, name: "Geometry invariants simulation only", timestamp: "2026-09-28T00:00:00Z"});
  const spec = {tool: {number: 1, offset: 2}, workOffset: 1, spindle, coolant: "flood"}, feed = {mode: "perMinute", value: 4 * scale};
  const s = p.addSection({...spec, name: "Quadrant arcs", start: a});
  // CW 270 degrees crosses the angular wrap; the CCW return is also 270.
  s.arc(b, c, true, feed, false, plane); s.arc(a, c, false, feed, false, plane);
  s.arc(a, c, true, feed, true, plane); s.arc(a, c, false, feed, true, plane);
  s.linear(repeat, feed); s.linear(a, feed);
  s.paths.at(-1).points.splice(1, 0, [...a]); // Explicit repeated source vertex.
  s.dwell(0.25);
  p.addSection({...spec, name: "Reviewed work link", start: link}).linear(end, {mode: "perRevolution", value: 0.01 * scale});
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text)), axes = machine === "mill" ? [0,1,2] : [0,2];
  plan.tools["1:2"] = {tool: 1, offset: 2};
  const retract = machine === "mill" ? [{z: 40 * scale}, {x: 0}, {y: 0}] : [{x: 40 * scale}, {z: 40 * scale}];
  plan.sections[0] = {mode: "retract", retract, approach: axes.map(i => ({["xyz"[i]]: a[i]}))};
  plan.sections[1] = {mode: "link", moves: [{["xyz"[u]]: link[u]}]}; plan.end = retract;
  const words = q => axes.map(i => ` ${"XYZ"[i]}${q[i]}`).join("");
  const waypoint = q => "G0" + Object.entries(q).map(([axis, value]) => ` ${axis.toUpperCase()}${value}`).join("");
  const init = `${units === "mm" ? "G21" : "G20"} ${machine === "mill" ? "G17" : "G18"} G8 G90 G91.1 G40 G80 G94 G61`;
  const stop = ["M5 $0", "M9", "G97 S0 $0"], retractCode = retract.map(q => "G53 " + waypoint(q));
  // Hand-specified command order; do not call production output/conversion code.
  const reference = [init, "G92.1", ...stop, ...retractCode, "G49", "T1 M6", init, "G92.1", "G54", "G43 H2",
    ...axes.map(i => `G0 ${"XYZ"[i]}${a[i]}`),
    machine === "lathe" ? `G96 D1800 S${units === "mm" ? 30.48 : 100} $0` : "G97 S600 $0", "M3 $0", "M9", "M8", `G94 F${4 * scale}`];
  if (plane !== (machine === "mill" ? "XY" : "XZ")) reference.push({XY:"G17",XZ:"G18",YZ:"G19"}[plane]);
  for (const [g, from, to, circle] of [["G2",a,b,false],["G3",b,a,false],["G2",a,a,true],["G3",a,a,true]]) {
    reference.push(g + (circle ? "" : axes.filter(i => i !== fixed).map(i => ` ${"XYZ"[i]}${to[i]}`).join("")) +
      axes.filter(i => i !== fixed).map(i => ` ${"IJK"[i]}${c[i] - from[i]}`).join("") + (circle ? " P1" : ""));
  }
  reference.push("G1" + words(a), "G1" + words(repeat), "G1" + words(a), "G4 P0.25",
    `G0 ${"XYZ"[u]}${link[u]}`, `G95 F${0.01 * scale}`, "G1" + words(end), ...stop, ...retractCode, "G49", "G94", "M2", "");
  return {text, plan, reference: reference.join("\n"), scale, a, b, c, repeat, link, end,
    // Controller table/parameters are in site inches, independently of G20/G21.
    toolTable: `T1 P1 X1 Y${machine === "mill" ? 2 : 0} Z3\nT2 P2 X4 Y${machine === "mill" ? 5 : 0} Z6\n`,
    parameters: `5220 1\n5221 10\n5222 ${machine === "mill" ? 20 : 0}\n5223 30\n`};
}
// Test-only decoder for these absolute, principal-plane synthetic programs.
// Reads G-code text, not sourceMap, and deliberately retains redundant moves.
function decodedGeometry(code) {
  const result = {motions: [], feeds: [], spindles: [], dwells: []}, positions = {work: [null,null,null], machine: [null,null,null]};
  let plane, scale;
  for (const raw of code.split("\n")) {
    const line = raw.replace(/\([^)]*\)/g, ""), words = [...line.matchAll(/([A-Z])([-+]?\d+(?:\.\d*)?)/g)];
    const gs = words.filter(w => w[1] === "G").map(w => Number(w[2])), value = key => { const w = words.find(w => w[1] === key); return w ? Number(w[2]) : undefined; };
    if (gs.includes(20)) scale = 1; if (gs.includes(21)) scale = 25.4;
    for (const [g,p] of [[17,"XY"],[18,"XZ"],[19,"YZ"]]) if (gs.includes(g)) plane = p;
    if (value("F") !== undefined) result.feeds.push({mode: gs.includes(95) ? "rev" : "minute", value: value("F") / scale});
    if (gs.includes(96) || gs.includes(97)) result.spindles.push({mode: gs.includes(96) ? "css" : "rpm", value: gs.includes(96) ? value("S") * (scale === 1 ? 12 : 1000 / 25.4) : value("S"), cap: value("D")});
    if (gs.includes(4)) result.dwells.push(value("P"));
    const g = gs.find(n => [0,1,2,3].includes(n)); if (g === undefined) continue;
    const frame = gs.includes(53) ? "machine" : "work", from = [...positions[frame]], end = [...from];
    for (let i = 0; i < 3; i++) if (value("XYZ"[i]) !== undefined) end[i] = value("XYZ"[i]) / scale;
    const motion = {g, frame, end};
    if (g === 2 || g === 3) {
      motion.plane = plane; motion.center = [...from];
      for (const i of frames[plane]) motion.center[i] += (value("IJK"[i]) || 0) / scale;
      motion.turns = value("P") || 1;
    }
    positions[frame] = end; result.motions.push(motion);
  }
  return result;
}
module.exports = {geometryFixture, decodedGeometry, configurations, frames};
