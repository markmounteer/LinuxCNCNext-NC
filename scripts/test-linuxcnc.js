"use strict";
// Offline standalone interpreter only: no LinuxCNC task, HAL or controller connection.
const fs = require("node:fs"), path = require("node:path"), os = require("node:os"), assert = require("node:assert/strict");
const {spawnSync} = require("node:child_process"), {example, continuationExample, linkExample} = require("./example"), {translate} = require("../src/translate");
const root = path.resolve(__dirname, ".."), artifacts = path.join(root, "artifacts", "linuxcnc"); fs.mkdirSync(artifacts, {recursive: true});
const executable = process.env.LINUXCNC_RS274 || "rs274";
const toolTable = "T1 P1 X0 Z0 D0.8 I0 J0 Q1 ; synthetic\n";
for (const units of ["mm", "inch"]) {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-rs274-"));
  try {
    const {text, plan} = example(units), result = translate(text, plan, {toolTable});
    assert.equal(result.report.toolTable.status, "passed");
    const input = path.join(temporary, "sample.ngc"), tool = path.join(temporary, "tool.tbl"), vars = path.join(temporary, "rs274.var");
    fs.writeFileSync(input, result.gcode); fs.writeFileSync(tool, toolTable); fs.writeFileSync(vars, "");
    const run = spawnSync(executable, ["-t", tool, "-v", vars, "-n", "0", "-g", input], {cwd: temporary, encoding: "utf8", timeout: 30000});
    if (run.error) throw run.error;
    fs.writeFileSync(path.join(artifacts, units + ".ngc"), result.gcode);
    fs.writeFileSync(path.join(artifacts, units + ".trace.txt"), run.stdout + run.stderr);
    assert.equal(run.status, 0, run.stdout + run.stderr);
    assert.doesNotMatch(run.stdout + run.stderr, /(?:error|bad character|unknown word|near line)/i);
    assert.match(run.stdout, /ARC_FEED/); assert.match(run.stdout, /PROGRAM_END/);
    assert.equal((run.stdout.match(/ARC_FEED\(/g) || []).length, 3);
    assert.match(run.stdout, /SET_SPINDLE_MODE/);
    const arcs = [...run.stdout.matchAll(/ARC_FEED\(([^)]*)\)/g)].map(m => m[1].split(",").slice(0, 5).map(Number));
    // In the selected G18 plane LinuxCNC's canonical order is Z, X.
    // Validate actual interpreted endpoint/centre/sense, not just G-code text.
    assert.deepEqual(arcs, [[-2, 8, 0, 8, 1], [0, 10, 0, 8, -1], [0, 10, 0, 8, -1]]);
    const speeds = [...run.stdout.matchAll(/SET_SPINDLE_SPEED\(0,\s*([\d.]+)\)/g)].map(m => Number(m[1]));
    assert.ok(speeds.includes(units === "mm" ? 80 : 100)); assert.ok(speeds.includes(700));
    assert.match(run.stdout, /SET_SPINDLE_MODE\(0[, ]\s*1800\.0000\)/);
    assert.match(run.stdout, /SET_FEED_MODE\(0, 1\)/); assert.match(run.stdout, /SET_FEED_RATE\(0\.1000\)/);
    if (units === "mm") {
      const source = path.join(temporary, "source file.stpnc"), planFile = path.join(temporary, "reviewed plan.json");
      fs.writeFileSync(source, text); fs.writeFileSync(planFile, JSON.stringify(plan));
      const env = {...process.env, NEXTNC_PLAN: planFile, NEXTNC_TOOL_TABLE: tool, NEXTNC_DIAGNOSTICS: path.join(temporary, "reports")};
      const filtered = spawnSync("sh", [path.join(root, "bin/nextnc-filter"), source], {env, encoding: "utf8", timeout: 30000});
      assert.equal(filtered.status, 0, filtered.stderr); assert.equal(filtered.stdout, result.gcode);
      fs.writeFileSync(tool, "T9 P9\n");
      const rejected = spawnSync("sh", [path.join(root, "bin/nextnc-filter"), source], {env, encoding: "utf8", timeout: 30000});
      assert.equal(rejected.status, 1); assert.equal(rejected.stdout, ""); assert.match(rejected.stderr, /TOOL_TABLE_MISSING/);
    }
    console.log(run.stdout.split("\n").filter(line => /ARC_FEED|SET_SPINDLE_MODE|SET_SPINDLE_SPEED|SET_FEED_MODE|SET_FEED_RATE/.test(line)).join("\n"));
    console.log(`PASS: LinuxCNC rs274 ${units}, lines/arcs/full circle/CSS/G95/dwell/tool offsets.`);
  } finally { fs.rmSync(temporary, {recursive: true, force: true}); }
}
for (const mode of ["continue", "link"]) for (const units of ["mm", "inch"]) {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-connection-rs274-"));
  try {
    const {text, plan} = (mode === "link" ? linkExample : continuationExample)(units), result = translate(text, plan, {toolTable});
    const input = path.join(temporary, "sample.ngc"), tool = path.join(temporary, "tool.tbl"), vars = path.join(temporary, "rs274.var");
    fs.writeFileSync(input, result.gcode); fs.writeFileSync(tool, toolTable); fs.writeFileSync(vars, "");
    const run = spawnSync(executable, ["-t", tool, "-v", vars, "-n", "0", "-g", input], {cwd: temporary, encoding: "utf8", timeout: 30000});
    if (run.error) throw run.error;
    fs.writeFileSync(path.join(artifacts, units + "-" + mode + ".ngc"), result.gcode);
    fs.writeFileSync(path.join(artifacts, units + "-" + mode + ".trace.txt"), run.stdout + run.stderr);
    assert.equal(run.status, 0, run.stdout + run.stderr);
    assert.doesNotMatch(run.stdout + run.stderr, /(?:error|bad character|unknown word|near line)/i);
    assert.match(run.stdout, /PROGRAM_END/);
    const feeds = [...run.stdout.matchAll(/STRAIGHT_FEED\(([^)]*)\)/g)].map(m => m[1].split(",").slice(0, 3).map(Number));
    assert.deepEqual(feeds, [[10, 0, -1], [10, 0, -2]]);
    assert.equal((run.stdout.match(/START_SPINDLE_CLOCKWISE\(/g) || []).length, 1);
    assert.match(run.stdout, /SET_FEED_RATE\(0\.1800\)/); assert.match(run.stdout, /SET_FEED_RATE\(0\.0800\)/);
    if (mode === "link") {
      const traverses = [...run.stdout.matchAll(/STRAIGHT_TRAVERSE\(([^)]*)\)/g)].map(m => m[1].split(",").slice(0, 3).map(Number));
      const index = traverses.findIndex(p => p[0] === 12 && p[1] === 0 && p[2] === 3);
      assert.ok(index >= 0, "First reviewed link waypoint was interpreted.");
      assert.deepEqual(traverses.slice(index, index + 3), [[12, 0, 3], [14, 0, 3], [14, 0, 2]]);
    }
    console.log(run.stdout.split("\n").filter(line => /STRAIGHT_FEED|START_SPINDLE|SET_FEED_RATE/.test(line)).join("\n"));
    console.log(`PASS: LinuxCNC rs274 ${units} ${mode}, one spindle start, exact cutting endpoints/feeds and reviewed waypoint order.`);
  } finally { fs.rmSync(temporary, {recursive: true, force: true}); }
}
