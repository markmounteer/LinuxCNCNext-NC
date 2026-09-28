"use strict";
// Offline standalone interpreter; synthetic jobs only, no controller connection.
const fs = require("node:fs"), path = require("node:path"), os = require("node:os"), assert = require("node:assert/strict");
const {spawnSync} = require("node:child_process"), {translate} = require("../src/translate");
const {semanticFixture, referenceGcode} = require("../test/support/semantic-fixture");
const {canonicalEvents, compareEvents} = require("../test/support/canonical");
const artifacts = path.resolve(__dirname, "../artifacts/linuxcnc"); fs.mkdirSync(artifacts, {recursive: true});
function runProgram(label, gcode, fixture) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-semantics-"));
  try {
    fs.writeFileSync(path.join(dir, "input.ngc"), gcode);
    fs.writeFileSync(path.join(dir, "tool.tbl"), fixture.toolTable);
    fs.writeFileSync(path.join(dir, "rs274.var"), fixture.parameters);
    // Standalone rs274 otherwise defaults external (machine/table) units to
    // inches, independently of the program's G20/G21. Pin this synthetic site.
    const ini = "[TRAJ]\nLINEAR_UNITS = mm\n";
    fs.writeFileSync(path.join(dir, "machine.ini"), ini);
    fs.writeFileSync(path.join(artifacts, label + ".ngc"), gcode);
    fs.writeFileSync(path.join(artifacts, label + ".tool.tbl"), fixture.toolTable);
    fs.writeFileSync(path.join(artifacts, label + ".var"), fixture.parameters);
    fs.writeFileSync(path.join(artifacts, label + ".ini"), ini);
    const run = spawnSync(process.env.LINUXCNC_RS274 || "rs274", ["-i", "machine.ini", "-t", "tool.tbl", "-v", "rs274.var", "-n", "0", "-g", "input.ngc"], {cwd: dir, encoding: "utf8", timeout: 30000});
    const trace = (run.stdout || "") + (run.stderr || ""); fs.writeFileSync(path.join(artifacts, label + ".trace.txt"), trace);
    if (run.error) throw run.error;
    return {trace, events: canonicalEvents(trace), status: run.status};
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
}
for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
  const name = units + "-" + machine + "-semantic", fixture = semanticFixture(machine, units);
  const result = translate(fixture.text, fixture.plan, {toolTable: fixture.toolTable});
  const expected = runProgram(name + "-reference", referenceGcode(machine, units), fixture);
  const actual = runProgram(name, result.gcode, fixture);
  for (const run of [expected, actual]) { assert.equal(run.status, 0, run.trace); assert.doesNotMatch(run.trace, /(?:error|bad character|unknown word|near line)/i); }
  compareEvents(actual.events, expected.events);
  assert.ok(actual.events.some(e => e.type === "PROGRAM_END"));
  // Independent physical-offset assertions: machine table is mm, even when
  // the program is inch. T1 uses H3, T2 uses H1, and G54 differs from G55.
  const scale = units === "mm" ? 1 : 25.4;
  for (const offset of [[4, machine === "mill" ? 5 : 0, 6], [1, machine === "mill" ? 2 : 0, 3]]) {
    assert.ok(actual.events.some(e => e.type === "USE_TOOL_LENGTH_OFFSET" && offset.every((v,i) => Math.abs(e.args[i] - v / scale) < 0.00011)), "Nonzero independent H offsets");
  }
  assert.ok(actual.events.some(e => e.type === "SET_G5X_OFFSET" && e.args[0] === 2 && Math.abs(e.args[1] - 40 / scale) < 0.00011), "Nonzero G55");
  const corruptions = {
    plane: code => code.replace(machine === "mill" ? "G19\n" : "G18 G8", machine === "mill" ? "G17\n" : "G17 G8"),
    center: code => code.replace(machine === "mill" ? "J-2 K0" : "I-2 K0", machine === "mill" ? "J-1 K0" : "I-1 K0"),
    feedMode: code => code.replace("G95 F0.2", "G94 F0.2"),
    spindle: code => code.replace("M4 $0", "M3 $0"),
    offset: code => code.replace("G43 H3", "G43 H2")
  };
  for (const [kind, change] of Object.entries(corruptions)) {
    const changed = change(result.gcode); assert.notEqual(changed, result.gcode);
    const bad = runProgram(name + "-corrupt-" + kind, changed, fixture);
    // A corrupt plane/center may be rejected by rs274. Valid but wrong state
    // must differ from the independent canonical trace instead.
    if (bad.status === 0 && !/(?:error|bad character|unknown word|near line)/i.test(bad.trace)) assert.throws(() => compareEvents(bad.events, expected.events), {name: "AssertionError"});
  }
  console.log(`PASS: ${machine} ${units}: complete ordered canonical events, nonzero T/H/WCS, continue/link/retract, reversal/feed/coolant and five corruptions.`);
}
