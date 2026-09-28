"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const fs = require("node:fs"), os = require("node:os"), path = require("node:path"), {spawnSync} = require("node:child_process");
const {readProgram} = require("../src/profile"), {translate} = require("../src/translate");
const {semanticFixture} = require("./support/semantic-fixture"), {renderReport} = require("../src/report");
const {correctionFor} = require("../src/errors");
const {Program} = require("../vendor/fusion360next-nc/next-nc");
for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
  test(`${machine}/${units}: metadata type failures identify the record and qualified attribute before decoding`, () => {
    const {text, plan} = semanticFixture(machine, units);
    for (const [entity, attribute] of [["APPLICATION_CONTEXT", "APPLICATION_CONTEXT.application"], ["CARTESIAN_POINT", "REPRESENTATION_ITEM.name"], ["PRODUCT", "PRODUCT.id"]]) {
      const malformed = text.replace(new RegExp(`(${entity}\\()'[^']*'`), "$142.");
      assert.notEqual(malformed, text);
      const line = malformed.split("\n").findIndex(l => l.includes(`${entity}(42.`));
      assert.throws(() => translate(malformed, plan), error => {
        assert.equal(error.code, "PROFILE_SHAPE");
        assert.deepEqual(error.context, {stage: "profile-shape", record: malformed.split("\n")[line].split("=")[0], sourceLine: line + 1, sourceColumn: 1, component: entity, attribute, parameterIndex: 1, expected: "string", actual: "number"});
        assert.equal(error.validationCoverage.stages.part21.status, "passed");
        assert.equal(error.validationCoverage.stages.profileShape.status, "failed");
        assert.equal(error.validationCoverage.stages.semantics.status, "not_checked");
        assert.doesNotMatch(correctionFor(error), /execution-plan|compensation|feed/i);
        return true;
      });
    }
  });
}
test("contract distinguishes required/optional/derived values, qualified inheritance, references and aggregate shapes", () => {
  const {text} = semanticFixture("lathe", "inch");
  const mutations = [
    ["APPLICATION_CONTEXT('Application protocol for the exchange of CNC data')", "APPLICATION_CONTEXT($)", "APPLICATION_CONTEXT.application"],
    ["NAMED_UNIT(*)", "NAMED_UNIT($)", "NAMED_UNIT.dimensions"],
    ["NAMED_UNIT(*)", "NAMED_UNIT(#1)", "NAMED_UNIT.dimensions"],
    ["INSTANCED_FEATURE('','toolpath','','toolpath'", "INSTANCED_FEATURE('','toolpath',42.,'toolpath'", "SHAPE_ASPECT.name"],
    ["INSTANCED_FEATURE('','toolpath','','toolpath'", "INSTANCED_FEATURE('','toolpath','',42.", "SHAPE_ASPECT.description"],
    ["ACTION_PROPERTY_REPRESENTATION('','',", "ACTION_PROPERTY_REPRESENTATION('','',42.,", "<extra parameter>"],
    ["REPRESENTATION('',(#", "REPRESENTATION('',#", "PARSE"],
    ["TIME_MEASURE(60.)", "TIME_MEASURE('60')", "MEASURE_WITH_UNIT.value_component"],
    ["SI_UNIT($,.SECOND.)", "SI_UNIT(*,.SECOND.)", "SI_UNIT.prefix"]
  ];
  for (const [from, to, attribute] of mutations) {
    assert.ok(text.includes(from), from);
    assert.throws(() => readProgram(text.replace(from, to)), e => attribute === "PARSE" ? e.code === "INVALID_NEXTNC" : e.code === "PROFILE_SHAPE" && e.context.attribute === attribute, attribute);
  }
  const wrongRef = text.replace(/(ACTION_PROPERTY_REPRESENTATION\('','',)#\d+/, "$1#1");
  assert.throws(() => readProgram(wrongRef), e => e.context.attribute === "ACTION_PROPERTY_REPRESENTATION.property" && e.context.expected.includes("ACTION_PROPERTY"));
  const shortPoint = text.replace(/(CARTESIAN_POINT\('[^']*',)\([^)]*\)/, "$1(1.,2.)");
  assert.throws(() => readProgram(shortPoint), e => e.context.actual === "aggregate[2]" && e.context.expected === "aggregate[3:3] of finite number");
  assert.throws(() => readProgram(text.replace("LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.)", "LENGTH_UNIT() NAMED_UNIT(*) TIME_UNIT() SI_UNIT(.MILLI.,.METRE.)")), e => e.code === "PROFILE_SHAPE" && e.context.attribute === "<components>");
  const conversion = text.replace(/(CONVERSION_BASED_UNIT\('inch',#\d+\) LENGTH_UNIT\(\) NAMED_UNIT\()#\d+/, "$1*");
  assert.throws(() => readProgram(conversion), e => e.context.attribute === "NAMED_UNIT.dimensions" && e.context.actual === "*");
  const optional = text.replace(/(PRODUCT\('workpiece','[^']*',)\$/, "$1''").replace("INSTANCED_FEATURE('','toolpath','','toolpath'", "INSTANCED_FEATURE('',$,'',$");
  assert.deepEqual(readProgram(optional).report.programFingerprint, readProgram(text).report.programFingerprint);
});
test("Unicode, quoted and backslash metadata remain intact", () => {
  const p = new Program({machine: "mill", units: "mm", name: "O'Brien \\ Ω 😀"});
  p.addSection({name: "切削", tool: {number: 1, offset: 1}, workOffset: 1, start: [1, 2, 3], spindle: {mode: "rpm", speed: 500, clockwise: true}, coolant: "off"}).linear([2, 3, 4], {mode: "perMinute", value: 20});
  const result = readProgram(p.toSTEP()); assert.equal(result.model.name, p.name); assert.equal(result.model.sections[0].name, "切削");
});
test("coverage comes from completed validators and leaves unrun stages explicitly unchecked", () => {
  const {text, plan, toolTable} = semanticFixture("mill", "mm"), inspection = readProgram(text).report;
  const original = inspection.programFingerprint;
  assert.equal(inspection.validationCoverage.stages.profileShape.contract, "next-nc/profile-shape/1");
  assert.ok(inspection.validationCoverage.stages.profileShape.attributes > 500);
  assert.equal(inspection.validationCoverage.stages.executionPlan.status, "not_checked");
  for (const supplied of [undefined, toolTable]) {
    const report = translate(text, plan, {toolTable: supplied}).report, stages = report.validationCoverage.stages;
    for (const stage of ["part21", "profileShape", "semantics", "executionPlan", "completeness", "serialization"]) assert.equal(stages[stage].status, "passed");
    assert.equal(stages.toolTable.status, supplied ? "passed" : "not_checked");
    for (const stage of ["expressSchema", "expressAttributes", "entityWhere", "typeWhere", "uniqueness", "globalRules", "fullAP238"]) assert.equal(stages[stage].status, "not_checked");
    assert.deepEqual(report.programFingerprint, original);
  }
  for (const [run, failed] of [[() => readProgram(text.slice(0,-15)), "part21"], [() => translate(text, {}), "executionPlan"], [() => translate(text, plan, {toolTable: "T8 P8"}), "toolTable"]]) {
    assert.throws(run, e => e.validationCoverage.stages[failed].status === "failed" && e.validationCoverage.stages.serialization.status === "not_checked");
  }
  const record = {schema: "linuxcnc-next-nc/diagnostic/1", status: "failed"};
  assert.match(renderReport(record), /Validation coverage<\/h2><p>Not recorded/);
  inspection.validationCoverage.stages.profileShape.scope = '<img src=x onerror="alert(1)">';
  const html = renderReport({...record, inspection});
  assert.ok(html.includes("&lt;img")); assert.doesNotMatch(html, /<img/); assert.match(html, /globalRules/); assert.match(html, /Not checked/);
});
test("shape failures are archived with coverage and publish no output from translate, preflight or filter", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-shape-"));
  try {
    const {text, plan} = semanticFixture("lathe", "mm"), input = path.join(dir, "job.stpnc"), planPath = path.join(dir, "plan.json"), diagnostics = path.join(dir, "diagnostics"), output = path.join(dir, "job.ngc");
    fs.writeFileSync(input, text.replace("APPLICATION_CONTEXT('Application protocol for the exchange of CNC data')", "APPLICATION_CONTEXT(42.)")); fs.writeFileSync(planPath, JSON.stringify(plan));
    for (const command of ["translate", "preflight", "filter"]) {
      const args = [path.join(__dirname, "../bin/nextnc.js"), command, input, ...(command === "filter" ? [] : ["--plan", planPath, "--output", output])];
      const run = spawnSync(process.execPath, args, {encoding: "utf8", env: {...process.env, NEXTNC_PLAN: planPath, NEXTNC_DIAGNOSTICS: diagnostics}});
      assert.equal(run.status, 1, run.stderr); assert.equal(run.stdout, ""); assert.equal(fs.existsSync(output), false);
      const archive = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json")));
      assert.equal(archive.error.code, "PROFILE_SHAPE"); assert.equal(archive.inspection.validationCoverage.stages.profileShape.status, "failed");
    }
    fs.writeFileSync(output, "preserve existing");
    const run = spawnSync(process.execPath, [path.join(__dirname, "../bin/nextnc.js"), "translate", input, "--plan", planPath, "--output", output], {encoding: "utf8", env: {...process.env, NEXTNC_DIAGNOSTICS: diagnostics}});
    assert.equal(run.status, 1); assert.equal(run.stdout, ""); assert.equal(fs.readFileSync(output, "utf8"), "preserve existing");
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
