"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const {spawnSync} = require("node:child_process"), {example, linkExample} = require("../scripts/example");
test("CLI/filter is all-or-nothing, preserves inputs and archives detailed failures locally", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "linuxcnc-nextnc-test-"));
  try {
    const input = path.join(dir, "input.stpnc"), planFile = path.join(dir, "plan.json"), output = path.join(dir, "output.ngc"), diagnostics = path.join(dir, "diagnostics");
    const {text, plan} = example(); fs.writeFileSync(input, text); fs.writeFileSync(planFile, JSON.stringify(plan));
    function run(...args) { return spawnSync(process.execPath, [path.join(__dirname, "../bin/nextnc.js"), ...args], {encoding: "utf8", env: {...process.env, NEXTNC_DIAGNOSTICS: diagnostics}}); }
    let r = run("translate", input); assert.equal(r.status, 1); assert.equal(r.stdout, ""); assert.match(r.stderr, /PLAN_REQUIRED/);
    const failure = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json"))); assert.equal(failure.error.code, "PLAN_REQUIRED"); assert.equal(failure.inputSHA256.length, 64);
    r = run("translate", input, "--plan", planFile); assert.equal(r.status, 0, r.stderr); assert.match(r.stdout, /^\(LinuxCNCNext-NC/); assert.ok(r.stdout.endsWith("M2\n"));
    assert.equal(JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json"))).error.code, "PLAN_REQUIRED");
    const success = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest.json"))); assert.equal(success.status, "succeeded"); assert.ok(success.sourceMap.some(p => p.operation && p.path));
    r = run("translate", input, "--plan", planFile, "--output", output); assert.equal(r.status, 0, r.stderr); assert.equal(r.stdout, "");
    const preserved = fs.readFileSync(output, "utf8"); r = run("translate", input, "--plan", planFile, "--output", output);
    assert.equal(r.status, 1); assert.equal(r.stdout, ""); assert.equal(fs.readFileSync(output, "utf8"), preserved);
    r = run("translate", input, "--plan", planFile, "--output", input); assert.equal(r.status, 1); assert.equal(fs.readFileSync(input, "utf8"), text);
    r = run("plan-template", input); assert.equal(r.status, 0); assert.equal(JSON.parse(r.stdout).sections[0].approach, null);
    plan.sections[0].approach[1].x = 999; fs.writeFileSync(planFile, JSON.stringify(plan)); r = run("translate", input, "--plan", planFile);
    assert.equal(r.status, 1); assert.equal(r.stdout, ""); const error = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json"))).error;
    assert.equal(error.code, "ENTRY_MISMATCH"); assert.equal(error.context.section, 1); assert.ok(error.context.operation);
    const linked = linkExample(); fs.writeFileSync(input, linked.text);
    linked.plan.sections[1].moves = [{x: 13}]; fs.writeFileSync(planFile, JSON.stringify(linked.plan));
    r = run("translate", input, "--plan", planFile);
    assert.equal(r.status, 1); assert.equal(r.stdout, "");
    const linkError = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json"))).error;
    assert.equal(linkError.code, "LINK_ENDPOINT"); assert.equal(linkError.context.section, 2);
    linked.plan.sections[1].moves = [{x: 14}]; fs.writeFileSync(planFile, JSON.stringify(linked.plan));
    r = run("translate", input, "--plan", planFile); assert.equal(r.status, 0, r.stderr);
    const linkedReport = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest.json")));
    assert.equal(linkedReport.inspection.execution.links, 1);
    assert.ok(linkedReport.sourceMap.some(p => p.phase === "link" && p.waypoint === 1));
    for (const args of [[], ["translate", input, "--bogus"], ["inspect", input, "--plan", planFile], ["translate", input, "--output"]]) assert.equal(run(...args).status, 2);
    const invalid = linked.text.replace("\nENDSEC;\nEND-ISO", "\n#999999=REPRESENTATION('',(#999998),#1);\nENDSEC;\nEND-ISO");
    fs.writeFileSync(input, invalid); r = run("translate", input, "--plan", planFile);
    assert.equal(r.status, 1); assert.equal(r.stdout, "");
    const parseError = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json"))).error;
    assert.equal(parseError.code, "INVALID_NEXTNC"); assert.equal(parseError.context.record, "#999999");
    assert.equal(parseError.context.sourceLine, invalid.split("\n").findIndex(l => l.startsWith("#999999=")) + 1);
    fs.writeFileSync(input, linked.text.replace("FILE_SCHEMA(('INTEGRATED_CNC_SCHEMA'));", "FILE_SCHEMA(('OTHER'));\nFILE_SCHEMA(('INTEGRATED_CNC_SCHEMA'));"));
    r = run("translate", input, "--plan", planFile); assert.equal(r.status, 1); assert.equal(r.stdout, "");
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
