"use strict";
const fs = require("node:fs"), path = require("node:path");
const {Program} = require("../vendor/fusion360next-nc/next-nc"), {readProgram} = require("../src/profile");
const {simulationPlan, example} = require("./example"), {translate} = require("../src/translate"), {renderReport} = require("../src/report");
function stockExample(units = "mm") {
  const k = units === "mm" ? 1 : 1 / 25.4, xyz = p => p.map(n => n * k);
  const p = new Program({machine: "mill", name: "Synthetic stock preview only", units, timestamp: "2026-09-29T00:00:00Z"});
  const s = p.addSection({name: "Slot and circular path", tool: {number: 1, offset: 1}, workOffset: 1, start: xyz([-4,0,2]), spindle: {mode: "rpm", speed: 1200, clockwise: true}, coolant: "flood"});
  const feed = {mode: "perMinute", value: 120 * k};
  s.linear(xyz([-4,0,-1]), feed); s.linear(xyz([4,0,-1]), feed); s.rapid(xyz([4,0,2]));
  s.rapid(xyz([2,0,2])); s.linear(xyz([2,0,-1]), feed); s.arc(xyz([2,0,-1]), xyz([0,0,-1]), false, feed, true, "XY"); s.rapid(xyz([2,0,2]));
  const text = p.toSTEP(), plan = simulationPlan(readProgram(text)), out = translate(text, plan);
  const setup = {schema: "linuxcnc-next-nc/stock-setup/1", machine: "mill", units,
    programFingerprint: out.report.programFingerprint.value, gcodeSHA256: out.report.traceability.gcodeSHA256, workOffset: "G54",
    stock: {min: xyz([-5,-3,-3]), max: xyz([5,3,0])}, tools: {1: {shape: "flat-end", diameter: 2 * k, cuttingLength: 8 * k}}, resolution: 0.125 * k};
  return {text, plan, out, setup};
}
function archive(out) { return {schema: "linuxcnc-next-nc/diagnostic/1", status: "succeeded", command: "preflight", translator: require("../package.json").version, inspection: out.report, sourceMap: out.sourceMap}; }
if (require.main === module) {
  const dir = path.resolve(__dirname, "../artifacts/review-examples"); fs.mkdirSync(dir, {recursive: true});
  const f = stockExample(), lathe = example(), l = translate(lathe.text, lathe.plan);
  fs.writeFileSync(path.join(dir, "mill.stpnc"), f.text); fs.writeFileSync(path.join(dir, "mill.ngc"), f.out.gcode);
  for (const [name, data] of [["mill-plan", f.plan], ["stock-setup", f.setup], ["mill-diagnostic", archive(f.out)], ["lathe-diagnostic", archive(l)]]) fs.writeFileSync(path.join(dir, name + ".json"), JSON.stringify(data, null, 2) + "\n");
  fs.writeFileSync(path.join(dir, "mill-review.html"), renderReport(archive(f.out), {stockSetup: f.setup}));
  fs.writeFileSync(path.join(dir, "lathe-review.html"), renderReport(archive(l)));
  console.log("Synthetic offline review examples: " + dir);
}
module.exports = {stockExample, archive};
