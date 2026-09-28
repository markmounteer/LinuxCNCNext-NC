"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const {spawnSync} = require("node:child_process");
const {example, continuationExample, millingExample} = require("../scripts/example");
const {translate} = require("../src/translate"), {readProgram} = require("../src/profile"), {template, validatePlan} = require("../src/plan");
const {renderReport} = require("../src/report"), {resolveConfiguration} = require("../src/configuration"), {doctor} = require("../src/doctor");
test("preflight aggregates independent plan fields and marks dependent checks not completed", () => {
  const {text} = continuationExample(), program = readProgram(text), plan = template(program);
  assert.throws(() => validatePlan(plan, program), error => {
    assert.equal(error.code, "TOOL_MAPPING"); assert.equal(error.context.section, 1);
    assert.deepEqual(error.context.issues.map(x => x.context.field), ["tools.1:1", "workOffsets.1", "tools.1:1", "workOffsets.1", "sections[0].retract", "sections[0].approach", "end"]);
    assert.equal(error.context.notChecked.length, 1); assert.equal(error.context.notChecked[0].section, 2);
    return true;
  });
  plan.programFingerprint = "wrong";
  assert.throws(() => validatePlan(plan, program), e => e.code === "PLAN_MISMATCH" && !e.context.issues);
  const valid = continuationExample().plan;
  Object.defineProperty(valid.tools, "1:1", {get() {throw new TypeError("Unexpected programming failure");}});
  assert.throws(() => validatePlan(valid, program), TypeError);
});
test("HTML renders lathe/mill archives and old failures without executing hostile text", () => {
  for (const fixture of [example, millingExample]) {
    const {text, plan} = fixture(), out = translate(text, plan), record = {schema: "linuxcnc-next-nc/diagnostic/1", status: "succeeded", command: "preflight", inspection: out.report, sourceMap: out.sourceMap};
    const before = JSON.stringify(record), html = renderReport(record);
    assert.match(html, /Tools in use/); assert.match(html, /href="#operation-0"/); assert.match(html, /Preflight computes candidate lines but writes no G-code/);
    assert.ok(html.includes(out.report.traceability.gcodeSHA256)); assert.equal(JSON.stringify(record), before);
    assert.match(html, /stateChange/); assert.match(html, /modalState/); assert.match(html, /spindleDirection/);
  }
  const hostile = '</pre><script>alert("x")</script><img src="https://example.com/pixel">';
  const html = renderReport({schema: "linuxcnc-next-nc/diagnostic/1", input: hostile, error: {message: hostile}});
  assert.doesNotMatch(html, /<script|<img|<iframe/i); assert.match(html, /&lt;script&gt;/); assert.match(html, /Not recorded/); assert.match(html, /default-src 'none'/);
  assert.throws(() => renderReport({schema: "linuxcnc-next-nc/diagnostic/2"}), e => e.code === "REPORT_SCHEMA");
  const diagnostic = renderReport({schema: "linuxcnc-next-nc/diagnostic/1", error: {code: "INVALID_NEXTNC", message: "arc endpoint radius mismatch", context: {stage: "geometry", rule: "ARC_RADIUS", operation: "Facing", path: 3, record: "#123", sourceLine: 131, mismatch: 0.01, threshold: 0.000002}}});
  assert.match(diagnostic, /Review the named Fusion operation/); assert.match(diagnostic, /ARC_RADIUS/); assert.match(diagnostic, /0.000002/);
});
test("doctor/filter share configuration precedence; report is exclusive and leaves job diagnostics unchanged", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc review "));
  try {
    const config = resolveConfiguration({HOME: dir, XDG_CONFIG_HOME: path.join(dir, "config"), XDG_STATE_HOME: path.join(dir, "state")}, "linux", dir);
    assert.equal(config.plan.path, path.join(dir, "config", "LinuxCNCNext-NC", "plan.json"));
    const {text, plan} = millingExample(), input = path.join(dir, "mill part.stpnc"), planPath = path.join(dir, "reviewed plan.json"), diagnostics = path.join(dir, "diagnostics"), table = path.join(dir, "tool table.tbl");
    fs.writeFileSync(input, text); fs.writeFileSync(planPath, JSON.stringify(plan)); fs.writeFileSync(table, "T1 P1\nT2 P2\nT3 P3\n");
    const env = {...process.env, NEXTNC_PLAN: planPath, NEXTNC_TOOL_TABLE: table, NEXTNC_DIAGNOSTICS: diagnostics};
    const resolved = resolveConfiguration(env), result = doctor(resolved);
    assert.equal(result.status, "passed"); assert.equal(fs.existsSync(diagnostics), false);
    assert.equal(resolved.plan.source, "NEXTNC_PLAN"); assert.equal(resolved.toolTable.source, "NEXTNC_TOOL_TABLE");
    const run = (...args) => spawnSync(process.execPath, [path.join(__dirname, "../bin/nextnc.js"), ...args], {encoding: "utf8", env});
    let r = run("doctor"); assert.equal(r.status, 0, r.stderr); assert.equal(JSON.parse(r.stdout).status, "passed");
    r = run("filter", input); assert.equal(r.status, 0, r.stderr); assert.equal(r.stdout, translate(text, plan).gcode);
    const latest = path.join(diagnostics, "latest.json"), before = fs.readFileSync(latest, "utf8"), output = path.join(dir, "review.html");
    r = run("report", latest, "--output", output); assert.equal(r.status, 0, r.stderr); assert.equal(r.stdout, ""); assert.equal(fs.readFileSync(latest, "utf8"), before);
    const html = fs.readFileSync(output, "utf8"); r = run("report", latest, "--output", output); assert.equal(r.status, 1); assert.equal(fs.readFileSync(output, "utf8"), html);
    fs.writeFileSync(planPath, "invalid json"); r = run("doctor"); assert.equal(r.status, 1); assert.equal(JSON.parse(r.stdout).status, "failed");
    r = run("filter", input); assert.equal(r.status, 1); assert.equal(r.stdout, "");
    const failure = path.join(diagnostics, "latest-error.json"), failedHtml = path.join(dir, "failed.html");
    assert.equal(run("report", failure, "--output", failedHtml).status, 0);
    for (const command of ["preflight", "translate"]) {
      fs.writeFileSync(planPath, JSON.stringify(template(readProgram(text))));
      r = run(command, input, "--plan", planPath); assert.equal(r.status, 1); assert.equal(r.stdout, "");
      assert.ok(JSON.parse(fs.readFileSync(failure)).error.context.issues.length > 5);
    }
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
