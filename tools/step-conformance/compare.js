"use strict";
const fs = require("node:fs"), path = require("node:path"), {createHash} = require("node:crypto"), assert = require("node:assert/strict");
const {parseP21} = require("./build/index.js");
const {parse} = require("../../vendor/fusion360next-nc/part21"), {readProgram} = require("../../src/profile");
const {normalizeUpstream, normalizeTarget, references, envelope, decode} = require("./normalize"), {cases} = require("./fixtures");
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const dir = path.resolve(__dirname, "../../artifacts/step-conformance"); fs.mkdirSync(dir, {recursive: true});
const evidence = {schema: "linuxcnc-next-nc/parser-comparison/1", upstream: require("./source.json"), build: require("./build-identity.json"),
  adapterSHA256: hash(fs.readFileSync(path.join(__dirname, "normalize.js"))), harnessSHA256: hash(fs.readFileSync(__filename)), fixturesSHA256: hash(fs.readFileSync(path.join(__dirname, "fixtures.js"))),
  target: {version: require("../../package.json").version, readerSHA256: hash(fs.readFileSync(path.join(__dirname, "../../vendor/fusion360next-nc/part21.js"))), contractSHA256: hash(fs.readFileSync(path.join(__dirname, "../../src/profile-contract.js"))), shapeValidatorSHA256: hash(fs.readFileSync(path.join(__dirname, "../../src/profile-shape.js")))},
  scope: "Synthetic accepted-profile parser comparison. No schema loader or general AP238 validation is run.",
  generalExpress: Object.fromEntries(["schemaParse", "schemaBuild", "instanceLoading", "attributes", "references", "entityWhere", "typeWhere", "uniqueness", "globalRules"].map(name => [name, {status: "not_checked", reason: "Parser-only harness; no EXPRESS schema loaded."}])), cases: []};
// Fixed answers detect adapter regressions without consulting the target decoder.
assert.equal(decode("'O''Brien \\\\ \\X2\\03A9D83DDE00\\X0\\'"), "O'Brien \\ Ω😀");
assert.throws(() => decode("'\\X2\\123\\X0\\'"));
let failures = 0;
function stage(fn) { try { return {status: "passed", value: fn()}; } catch (e) { return {status: "failed", error: {code: e.code, message: e.message, context: e.context}}; } }
const skipped = () => ({status: "not_checked"});
for (const fixture of cases()) {
  if (Buffer.byteLength(fixture.text) > 4 * 1024 * 1024) throw new Error("Fixture size limit");
  const input = fixture.name + ".stpnc"; fs.writeFileSync(path.join(dir, input), fixture.text);
  const record = {name: fixture.name, input, inputSHA256: hash(fixture.text), expected: fixture.expected, discrepancy: fixture.discrepancy};
  const oracle = parseP21(fixture.text, {maxEntities: 10000}); record.diagnostics = oracle.diagnostics;
  const stages = {syntax: {status: oracle.diagnostics.length ? "failed" : "passed"}};
  stages.envelope = stages.syntax.status === "passed" ? stage(() => envelope(fixture.text, oracle.ast)) : skipped();
  stages.normalization = stages.envelope.status === "passed" ? stage(() => normalizeUpstream(oracle.ast)) : skipped();
  stages.references = stages.normalization.status === "passed" ? stage(() => references(stages.normalization.value)) : skipped();
  stages.targetParse = stage(() => normalizeTarget(parse(fixture.text)));
  stages.targetProfile = stage(() => readProgram(fixture.text).report.validationCoverage);
  stages.agreement = stages.references.status === "passed" && stages.targetParse.status === "passed" ? stage(() => assert.deepEqual(stages.normalization.value, stages.targetParse.value)) : skipped();
  record.upstreamSpans = oracle.ast.data.flatMap(d => d.entities).map(e => ({id: e.id, span: e.span}));
  record.normalizedSHA256 = stages.normalization.value ? hash(JSON.stringify(stages.normalization.value)) : undefined;
  record.targetNormalizedSHA256 = stages.targetParse.value ? hash(JSON.stringify(stages.targetParse.value)) : undefined;
  record.targetCoverage = stages.targetProfile.value;
  record.stages = Object.fromEntries(Object.entries(stages).map(([name, {value, ...outcome}]) => [name, outcome]));
  const mismatch = Object.entries(fixture.expected).filter(([name, expected]) => name === "targetCode" ? stages.targetProfile.error?.code !== expected : stages[name].status !== expected);
  record.status = mismatch.length ? "failed" : "passed"; record.mismatch = mismatch;
  if (mismatch.length) { ++failures; console.error(fixture.name, JSON.stringify(mismatch), JSON.stringify(record.stages)); }
  evidence.cases.push(record);
  // Retain completed evidence even if a later upstream input crashes the worker.
  fs.writeFileSync(path.join(dir, "comparison.json"), JSON.stringify(evidence, null, 2) + "\n");
}
evidence.status = failures ? "failed" : "passed"; evidence.total = evidence.cases.length; evidence.failed = failures;
fs.writeFileSync(path.join(dir, "comparison.json"), JSON.stringify(evidence, null, 2) + "\n");
console.log(`${evidence.total} independent parser cases; ${failures} unexpected outcomes.`);
process.exitCode = failures ? 1 : 0;
