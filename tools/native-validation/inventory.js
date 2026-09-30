"use strict";
// Development-only AST inventory. This does not parse or admit machining jobs.
const fs = require("node:fs"), path = require("node:path"), crypto = require("node:crypto");
const acorn = require("acorn"), assert = require("node:assert/strict");
const root = path.resolve(__dirname, "../.."), normalized = s => s.replace(/\r\n/g, "\n");
const read = name => normalized(fs.readFileSync(path.join(root, name), "utf8"));
const hash = s => crypto.createHash("sha256").update(s).digest("hex");
const guards = new Set(["check", "recordCheck", "need", "invariant", "fail", "requireState"]);
const sources = [
  "vendor/fusion360next-nc/part21.js", "vendor/fusion360next-nc/inspect.js",
  "src/profile.js", "src/profile-shape.js", "src/continuity.js", "src/plan.js",
  "src/tool-table.js", "src/command-contract.js", "src/execution-audit.js",
  "src/policy-audit.js", "src/gcode-audit.js", "src/linuxcnc-output.js",
  "src/errors.js", "src/internal-error.js", "src/artifact-identity.js", "bin/nextnc.js"
];
function discover(file) {
  const source = read(file), tree = acorn.parse(source, {ecmaVersion: 2022, locations: true, allowHashBang: true});
  const result = [];
  function walk(node, owner = "<module>") {
    if (!node || typeof node !== "object") return;
    if (/Function/.test(node.type)) owner = node.id?.name || owner;
    const callee = node.type === "CallExpression" && node.callee.type === "Identifier" ? node.callee.name : null;
    const selected = guards.has(callee) || node.type === "ThrowStatement" ||
      file === "src/artifact-identity.js" && (node.type === "ReturnStatement" || node.type === "VariableDeclarator" && node.id.name === "valid") ||
      file === "src/policy-audit.js" && callee === "process" ||
      file === "src/continuity.js" && (callee === "no" || node.type === "ReturnStatement" && node.argument?.type === "ObjectExpression" && node.argument.properties.some(p => p.key.name === "eligible" && p.value.value === false));
    if (selected) result.push({id: `${file}:${node.loc.start.line}:${node.loc.start.column+1}`,
      file, line: node.loc.start.line, endLine: node.loc.end.line, owner,
      kind: callee || node.type, expression: source.slice(node.start, node.end)});
    for (const [key, value] of Object.entries(node)) {
      if (["loc", "start", "end"].includes(key)) continue;
      if (Array.isArray(value)) value.forEach(v => walk(v, owner));
      else if (value && typeof value === "object") walk(value, owner);
    }
  }
  walk(tree);
  return result;
}
function reference(spec) {
  const [file, anchor] = spec.split("#");
  const source = read(file), at = anchor ? source.indexOf(anchor) : 0;
  assert(at >= 0, `Missing reference ${spec}`);
  return {file, line: source.slice(0, at).split("\n").length, anchor: anchor || null, sha256: hash(source)};
}
function generate() {
  const review = require("./review");
  const checks = sources.flatMap(discover).map(site => {
    const choices = review.rules[site.file]?.filter(r => site.line >= r.lines[0] && site.line <= r.lines[1]) || [];
    assert.equal(choices.length, 1, `Expected one reviewed disposition for ${site.id}, found ${choices.length}`);
    const rule = choices[0];
    assert(["ported", "intentionally-changed", "missing"].includes(rule.disposition), `Bad disposition ${site.id}`);
    assert(rule.reason && rule.rust?.length && rule.tests?.length, `Incomplete evidence ${site.id}`);
    return {...site, disposition: rule.disposition, reason: rule.reason,
      rust: rule.rust.map(reference), tests: rule.tests.map(reference)};
  });
  // The dynamic grammar contains validation branches as DATA. Enumerate every
  // field and component combination instead of hiding them behind one guard.
  const legacy = JSON.parse(JSON.stringify(require(path.join(root, "src/profile-contract"))));
  const native = JSON.parse(read("src-rust/profile-shape.json"));
  assert.deepEqual(native, legacy, "Native closed grammar differs from the reviewed legacy grammar");
  const shape = [];
  for (const [entity, fields] of Object.entries(legacy.contract)) {
    shape.push({entity, arity: fields.length, disposition: "ported", fields: fields.map((f, i) => ({parameter: i+1, ...f, disposition: "ported"}))});
  }
  const catalog = ["src", "vendor/fusion360next-nc", "bin"].flatMap(dir =>
    fs.readdirSync(path.join(root, dir)).filter(n => n.endsWith(".js")).map(n => `${dir}/${n}`)).sort();
  const files = catalog.map(file => {
    const scope = sources.includes(file) ? "validation-sites" : review.otherFiles[file];
    assert(scope, `Unreviewed legacy module ${file}`);
    return {file, sha256: hash(read(file)), scope};
  });
  assert.deepEqual(Object.keys(review.otherFiles).sort(), catalog.filter(f => !sources.includes(f)), "Stale/missing module disposition");
  return {schema: "nextnc-native/validation-inventory/1", legacyRevision: review.legacyRevision,
    scope: "Every assertion/error site in the named legacy validation modules, every closed grammar field/component, and explicit disposition of the complete legacy module catalog. Evidence references are not a branch-coverage claim.",
    files, checks, shape, complexCombinations: legacy.complexCombinations,
    grammarEvidence: ["src-rust/shape.rs#pub fn validate", "src-rust/shape.rs#fn matches", "tests-rust/part21.rs#fn every_legacy_entity_arity_and_attribute_rejects_an_invalid_value"].map(reference)};
}
function markdown(data) {
  const link = r => `[${r.file}:${r.line}](../${r.file}#L${r.line})`;
  let out = "# Native validation check inventory\n\nGenerated from the reviewed dispositions in `tools/native-validation/review.js`.\nRun `npm --prefix tools/native-validation test` to detect stale source, changed\nchecks, missing dispositions, grammar drift or broken evidence references. This\nis a source-level migration inventory, not proof of complete branch coverage or\nphysical-machine qualification. Full expressions and hashes are in\n[native-validation-inventory.json](native-validation-inventory.json).\n\n";
  out += `Legacy checkpoint: \`${data.legacyRevision}\`. ${data.checks.length} assertion/error\nsites; ${data.shape.length} entity arities; ${data.shape.reduce((n,e)=>n+e.fields.length,0)} attribute constraints;\n${data.complexCombinations.length} complex-component combinations.\n\n`;
  for (const file of sources) {
    out += `## ${file}\n\n| Site | Disposition and reason | Rust implementation | Evidence tests |\n| --- | --- | --- | --- |\n`;
    for (const c of data.checks.filter(c=>c.file===file)) {
      out += `| [${c.line}](../${file}#L${c.line}): ${c.kind} in ${c.owner} | ${c.disposition}: ${c.reason.replace(/\|/g,"\\|")} | ${c.rust.map(link).join(", ")} | ${c.tests.map(link).join(", ")} |\n`;
    }
    out += "\n";
  }
  out += "## Data-driven grammar\n\nThe verifier compares the entire Rust grammar with the legacy contract. Each\nentity arity and every named attribute (including nested choices, reference target\ntypes, aggregates, bounds, symbols and context-sensitive dimensions) is enumerated\nin the JSON inventory. The same check compares all complex-component combinations.\nRust's recursive matcher and SI/non-SI dimensions dispatch enforce the same data.\n\n## Other module dispositions\n\n| Module | Scope |\n| --- | --- |\n";
  for (const f of data.files.filter(f=>f.scope!=="validation-sites")) out += `| ${f.file} | ${f.scope} |\n`;
  return out;
}
if (process.argv.includes("--discover")) {
  for (const file of sources) for (const s of discover(file)) console.log(`${s.id} ${s.owner} ${s.expression.replace(/\s+/g," ").slice(0,160)}`);
} else {
  const data = generate(), text = JSON.stringify(data, null, 2)+"\n", md = markdown(data);
  for (const [file, content] of [["docs/native-validation-inventory.json",text],["docs/native-validation-inventory.md",md]]) {
    if (process.argv.includes("--write")) fs.writeFileSync(path.join(root,file),content);
    else assert.equal(read(file), content, `Stale ${file}; review changes before regenerating`);
  }
  const missing = data.checks.filter(c=>c.disposition==="missing");
  assert.equal(missing.length,0,`Missing checks: ${missing.map(c=>c.id).join(", ")}`);
  console.log(JSON.stringify({status:"passed",sites:data.checks.length,entities:data.shape.length,attributes:data.shape.reduce((n,e)=>n+e.fields.length,0),combinations:data.complexCombinations.length,modules:data.files.length}));
}
