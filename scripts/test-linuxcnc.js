"use strict";
// Offline standalone interpreter only: no LinuxCNC task, HAL or controller connection.
const fs = require("node:fs"), path = require("node:path"), os = require("node:os"), assert = require("node:assert/strict");
const {spawnSync} = require("node:child_process"), {example} = require("./example"), {translate} = require("../src/translate");
const root = path.resolve(__dirname, ".."), artifacts = path.join(root, "artifacts", "linuxcnc"); fs.mkdirSync(artifacts, {recursive: true});
const executable = process.env.LINUXCNC_RS274 || "rs274";
for (const units of ["mm", "inch"]) {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-rs274-"));
  try {
    const {text, plan} = example(units), result = translate(text, plan);
    const input = path.join(temporary, "sample.ngc"), tool = path.join(temporary, "tool.tbl"), vars = path.join(temporary, "rs274.var");
    fs.writeFileSync(input, result.gcode); fs.writeFileSync(tool, "T1 P1 X0 Z0 D0.8 I0 J0 Q1 ; synthetic\n"); fs.writeFileSync(vars, "");
    const run = spawnSync(executable, ["-t", tool, "-v", vars, "-n", "0", "-g", input], {cwd: temporary, encoding: "utf8", timeout: 30000});
    if (run.error) throw run.error;
    fs.writeFileSync(path.join(artifacts, units + ".ngc"), result.gcode);
    fs.writeFileSync(path.join(artifacts, units + ".trace.txt"), run.stdout + run.stderr);
    assert.equal(run.status, 0, run.stdout + run.stderr);
    assert.doesNotMatch(run.stdout + run.stderr, /(?:error|bad character|unknown word|near line)/i);
    assert.match(run.stdout, /ARC_FEED/); assert.match(run.stdout, /PROGRAM_END/);
    assert.equal((run.stdout.match(/ARC_FEED\(/g) || []).length, 3);
    assert.match(run.stdout, /SET_SPINDLE_MODE/);
    console.log(run.stdout.split("\n").filter(line => /ARC_FEED|SET_SPINDLE_MODE|SET_SPINDLE_SPEED|SET_FEED_MODE|SET_FEED_RATE/.test(line)).join("\n"));
    console.log(`PASS: LinuxCNC rs274 ${units}, lines/arcs/full circle/CSS/G95/dwell/tool offsets.`);
  } finally { fs.rmSync(temporary, {recursive: true, force: true}); }
}
