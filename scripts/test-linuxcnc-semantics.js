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
    // Debian's 2023 standalone build fixes external units to inches; newer
    // rs274 can read TRAJ units. Use an explicit inch site for both versions,
    // independently of G20/G21, and assert its actual converted offsets below.
    const ini = "[TRAJ]\nLINEAR_UNITS = inch\n";
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
  // Independent physical-offset assertions: this synthetic machine/table is
  // inch, even when the program is mm. T1 uses H3, T2 H1; G54 differs from G55.
  const scale = units === "mm" ? 25.4 : 1;
  for (const offset of [[4, machine === "mill" ? 5 : 0, 6], [1, machine === "mill" ? 2 : 0, 3]]) {
    assert.ok(actual.events.some(e => e.type === "USE_TOOL_LENGTH_OFFSET" && offset.every((v,i) => Math.abs(e.args[i] - v * scale) < 0.00011)), "Nonzero independent H offsets in program units");
  }
  assert.ok(actual.events.some(e => e.type === "SET_G5X_OFFSET" && e.args[0] === 2 && Math.abs(e.args[1] - 40 * scale) < 0.00011), "Nonzero G55 in program units");
  const corruptions = {
    plane: code => machine === "mill" ? code.replace("G19\n", "G17\n") : code.replace(/^G3 /m, "G17\nG3 "),
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
// Representative quadrant/plane/unit cases, with independent explicit programs.
const {geometryFixture, configurations} = require("../test/support/geometry-fixture");
for (const [machine, plane] of configurations) for (const units of ["mm", "inch"]) {
  const fixture = geometryFixture(machine, plane, units, [-20,20]);
  const label = `${machine}-${plane}-${units}-geometry`, result = translate(fixture.text, fixture.plan, {toolTable: fixture.toolTable});
  const expected = runProgram(label + "-reference", fixture.reference, fixture), actual = runProgram(label, result.gcode, fixture);
  for (const run of [expected, actual]) { assert.equal(run.status, 0, run.trace); assert.doesNotMatch(run.trace, /(?:error|bad character|unknown word|near line)/i); }
  compareEvents(actual.events, expected.events);
  const arcs = actual.events.filter(e => e.type === "ARC_FEED");
  assert.equal(arcs.length, 4); assert.deepEqual(arcs.map(e => Math.sign(e.args[4])), [-1,1,-1,1]);
  const mutations = {
    scale: code => code.replaceAll("G21", "G20"),
    sense: code => code.replace(/^G2 /m, "G3 "),
    plane: code => code.replace(/^G2 /m, `${plane === "XY" ? "G18" : "G17"}\nG2 `),
    center: code => code.replace(/([IJK])(-?\d+(?:\.\d+)?)/, (_,w,n) => w + -Number(n)),
    frame: code => code.replace("G53 G0", "G0")
  };
  // Metric cases suffice to check all five fault types; inch cases still run
  // against complete independent reference programs, not endpoint-only checks.
  if (units === "mm") for (const [kind, mutate] of Object.entries(mutations)) {
    const badCode = mutate(result.gcode); assert.notEqual(badCode, result.gcode);
    const bad = runProgram(label + "-corrupt-" + kind, badCode, fixture);
    if (bad.status === 0 && !/(?:error|bad character|unknown word|near line)/i.test(bad.trace)) assert.throws(() => compareEvents(bad.events, expected.events), {name:"AssertionError"});
  }
  console.log(`PASS: ${machine} ${plane} ${units}: major arcs, both full circles, repeated/reversed vertices, work link and G53 frame isolation.`);
}
