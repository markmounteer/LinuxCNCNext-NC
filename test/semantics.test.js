"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), crypto = require("node:crypto");
const fs = require("node:fs"), path = require("node:path"), os = require("node:os"), {spawnSync} = require("node:child_process");
const examples = require("../scripts/example"), {translate} = require("../src/translate"), {readProgram} = require("../src/profile");
const {Program} = require("../vendor/fusion360next-nc/next-nc");
const {semanticFixture, referenceGcode} = require("./support/semantic-fixture");
const {assertSourceAssociations, canonicalEvents, compareEvents} = require("./support/canonical");
test("eight pre-refactor golden programs retain exact G-code and fingerprints", () => {
  for (const b of require("./emission-baseline.json")) {
    const f = examples[b.name](b.units), result = translate(f.text, f.plan);
    assert.equal(result.report.programFingerprint.value, b.fingerprint);
    assert.equal(crypto.createHash("sha256").update(result.gcode.split("\n").slice(1).join("\n")).digest("hex"), b.gcodeSHA256);
  }
});
test("both machines match a hand-authored oracle; modal/source state does not leak between calls", () => {
  for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
    const f = semanticFixture(machine, units), result = translate(f.text, f.plan);
    const uncomment = text => text.split("\n").filter(l => !l.startsWith("(")).join("\n");
    assert.equal(uncomment(result.gcode), referenceGcode(machine, units));
    assertSourceAssociations(readProgram(f.text), result);
    const feed = result.sourceMap.find(e => e.command.type === "feed" && e.command.mode === "perRevolution");
    assert.deepEqual(feed.stateChange.feedMode, {before: "perMinute", after: "perRevolution"});
    const cut = result.sourceMap.find(e => e.kind === "linear" && e.section === 3 && e.action === "motion");
    assert.equal(cut.modalState.feedMode, "perRevolution"); assert.equal(cut.modalState.feedRate, 0.2);
    assert.equal(cut.modalState.spindleDirection, "counterclockwise"); assert.equal(cut.modalState.coolant, "mist");
    assert.equal(cut.modalState.tool, 1); assert.equal(cut.modalState.toolOffset, 3);
    const firstRapid = result.sourceMap.find(e => e.command.type === "rapid");
    assert.equal(firstRapid.modalState.feedRate, 0);
    assert.equal(firstRapid.command.frame, "machine"); assert.ok(!firstRapid.motion.start);
    assert.ok(!firstRapid.stateChange.workPosition); // All work axes remain unknown.
    const toolChange = result.sourceMap.find(e => e.command.type === "toolChange");
    assert.deepEqual(toolChange.stateChange.machinePosition.after, [null, null, null]);
    const lastCut = result.sourceMap.find(e => e.section === 4 && e.action === "motion" && e.phase === "toolpath");
    assert.equal(lastCut.modalState.plane, machine === "mill" ? "XY" : "XZ");
    assert.equal(lastCut.modalState.tool, 2); assert.equal(lastCut.modalState.toolOffset, 1); assert.equal(lastCut.modalState.workOffset, "G55");
    assert.equal(result.sourceMap.at(-1).stateChange.ended.after, true);
    assert.equal(result.sourceMap.at(-1).stateChange.workOffset.after, null);
    translate(examples.example().text, examples.example().plan);
    assert.deepEqual(translate(f.text, f.plan), result);
    const corrupt = structuredClone(result); corrupt.sourceMap.find(e => e.action === "motion" && e.path).path += 1;
    assert.throws(() => assertSourceAssociations(readProgram(f.text), corrupt), /Decoded path\/segment association/);
  }
});
test("compacted and explicit polylines preserve every repeated and reversed vertex in both machines", () => {
  for (const machine of ["lathe", "mill"]) {
    const p = new Program({machine, units: "mm"}), s = p.addSection({start: [4, 0, 2], tool: {number: 1, offset: 1}, workOffset: 1, spindle: {mode: "rpm", speed: 600, clockwise: true}, coolant: "off"});
    s.linear([3, 0, 1], {mode: "perMinute", value: 80}); s.linear([4, 0, 2], {mode: "perMinute", value: 80}); s.linear([3, 0, 1], {mode: "perMinute", value: 80});
    // A duplicate vertex is a deliberate part of the input, not endpoint-only equivalence.
    s.paths[0].points.splice(2, 0, [3, 0, 1]);
    const compact = p.toSTEP(), original = s.paths[0];
    s.paths = original.points.slice(1).map((end, i) => ({...original, points: [original.points[i], end]}));
    const explicit = p.toSTEP();
    const a = translate(compact, examples.simulationPlan(readProgram(compact))), b = translate(explicit, examples.simulationPlan(readProgram(explicit)));
    const motions = r => r.sourceMap.filter(e => e.action === "motion" && e.phase === "toolpath").map(e => [e.motion.start, e.motion.end, e.modalState]);
    assert.deepEqual(motions(a), motions(b)); assert.equal(motions(a).length, 4);
    assertSourceAssociations(readProgram(compact), a); assertSourceAssociations(readProgram(explicit), b);
    // Duplicate a point definition for just one polyline use. Geometry sharing
    // is storage only: the existing reviewed plan and complete output must match.
    const curve = compact.match(/^#\d+=POLYLINE\('',\(#(\d+),/m), id = curve[1];
    const point = compact.split("\n").find(line => line.startsWith("#" + id + "="));
    const unshared = compact.replace(curve[0], curve[0].replace("(#" + id + ",", "(#999999,"))
      .replace("\nENDSEC;\nEND-ISO", "\n" + point.replace(/^#\d+=/, "#999999=") + "\nENDSEC;\nEND-ISO");
    const c = translate(unshared, examples.simulationPlan(readProgram(compact)));
    assert.equal(c.gcode, a.gcode); assert.deepEqual(c.sourceMap.map(({provenance, ...entry}) => entry), a.sourceMap.map(({provenance, ...entry}) => entry));
    assert.notDeepEqual(c.sourceMap, a.sourceMap);
  }
});
test("canonical comparison detects wrong plane, arc center, feed, spindle and offset events", () => {
  const trace = "1 N..... SELECT_PLANE(CANON_PLANE_XZ)\n2 N..... ARC_FEED(-2.0000, 8.0000, 0.0000, 8.0000, 1, 0.0000)\n3 N..... SET_FEED_MODE(0, 1)\n4 N..... START_SPINDLE_COUNTERCLOCKWISE(0)\n5 N..... USE_TOOL_LENGTH_OFFSET(4.0000 0.0000 6.0000, 0.0000 0.0000 0.0000, 0.0000 0.0000 0.0000)\n";
  const expected = canonicalEvents(trace); assert.equal(expected.length, 5);
  assert.deepEqual(canonicalEvents("6 N..... SET_G5X_OFFSET(2, 40.0000, 50.0000, 60.0000)\n"), [{type: "SET_G5X_OFFSET", args: [2, 40, 50, 60]}]);
  for (const change of [t => t.replace("PLANE_XZ", "PLANE_XY"), t => t.replace("0.0000, 8.0000", "1.0000, 8.0000"), t => t.replace("MODE(0, 1)", "MODE(0, 0)"), t => t.replace("COUNTERCLOCKWISE", "CLOCKWISE"), t => t.replace("OFFSET(4.0000", "OFFSET(7.0000")]) assert.throws(() => compareEvents(canonicalEvents(change(trace)), expected));
});
test("geometry failure archives agree across preflight/translate; formatter failure writes no G-code", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-errors-"));
  try {
    const input = path.join(dir, "input.stpnc"), planFile = path.join(dir, "plan.json"), output = path.join(dir, "candidate.ngc"), diagnostics = path.join(dir, "diagnostics");
    const run = (...args) => spawnSync(process.execPath, [path.join(__dirname, "../bin/nextnc.js"), ...args], {encoding: "utf8", env: {...process.env, NEXTNC_DIAGNOSTICS: diagnostics}});
    for (const factory of [examples.example, examples.millingExample]) {
      const f = factory(), bad = f.text.replace(/(CIRCLE\([^\n]*,)([\d.]+)(\);)/, (_,a,r,b) => a + (Number(r) + 1) + b);
      fs.writeFileSync(input, bad); fs.writeFileSync(planFile, JSON.stringify(f.plan));
      const failures = [];
      for (const command of ["preflight", "translate"]) {
        const r = run(command, input, "--plan", planFile, ...(command === "translate" ? ["--output", output] : []));
        assert.equal(r.status, 1, r.stderr); assert.equal(r.stdout, ""); assert.ok(!fs.existsSync(output));
        failures.push(JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json"))).error);
      }
      assert.deepEqual(failures[0], failures[1]); assert.equal(failures[0].code, "INVALID_NEXTNC"); assert.equal(failures[0].context.rule, "ARC_RADIUS");
    }
    const p = new Program({units: "mm"}); p.addSection({start: [1, 0, 1], tool: {number: 1, offset: 1}, workOffset: 1, spindle: {mode: "rpm", speed: 600, clockwise: true}, coolant: "off"}).linear([1e-200, 0, 0], {mode: "perMinute", value: 50});
    const text = p.toSTEP(); fs.writeFileSync(input, text); fs.writeFileSync(planFile, JSON.stringify(examples.simulationPlan(readProgram(text))));
    const r = run("translate", input, "--plan", planFile, "--output", output);
    assert.equal(r.status, 1); assert.equal(r.stdout, ""); assert.ok(!fs.existsSync(output)); assert.match(r.stderr, /PRECISION/);
    const fault = spawnSync(process.execPath, ["-e", 'const parser=require("./vendor/fusion360next-nc/part21");const fault=new TypeError("fault");parser.parse=()=>{throw fault;};const {readProgram}=require("./src/profile");try{readProgram("");process.exit(1);}catch(e){if(e!==fault)process.exit(2);}'], {cwd: path.join(__dirname, ".."), encoding: "utf8"});
    assert.equal(fault.status, 0, fault.stderr);
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
