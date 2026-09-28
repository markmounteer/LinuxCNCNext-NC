"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const fs = require("node:fs"), os = require("node:os"), path = require("node:path"), crypto = require("node:crypto");
const {spawnSync} = require("node:child_process");
const {translate} = require("../src/translate"), {readProgram} = require("../src/profile");
const {example, continuationExample, linkExample, simulationPlan} = require("../scripts/example");
const {Program} = require("../vendor/fusion360next-nc/next-nc");

test("tool-table snapshot checks mapped T and H records without altering geometry or plan", () => {
  const table = "\uFEFF; snapshot\r\nT0 P0 ; empty spindle\r\nT7 P1001 X-1.25 Z.3 D0.8 I95 J5 Q6\r\nT9 P9 X1e-3\r\n";
  for (const units of ["mm", "inch"]) {
    const {text, plan} = example(units); plan.tools["1:1"] = {tool: 7, offset: 9};
    const original = JSON.stringify(plan), without = translate(text, plan), out = translate(text, plan, {toolTable: table});
    assert.equal(without.report.toolTable.status, "not_checked");
    assert.equal(out.report.toolTable.status, "passed"); assert.equal(out.report.toolTable.records, 3);
    assert.equal(out.report.toolTable.mappings[0].offsetTableLine, 4);
    assert.equal(out.report.toolTable.sha256, crypto.createHash("sha256").update(table).digest("hex"));
    assert.equal(out.gcode, without.gcode); assert.equal(JSON.stringify(plan), original);
  }
});

test("missing selected tools and independent H records are aggregated across operations", () => {
  const {text, plan} = continuationExample(); plan.tools["1:1"] = {tool: 7, offset: 9};
  assert.throws(() => translate(text, plan, {toolTable: "T1 P1\n"}), error => {
    assert.equal(error.code, "TOOL_TABLE_MISSING");
    assert.deepEqual(error.context.operations.map(x => [x.section, x.operation, x.missing.map(m => m.record)]), [[1, "First", [7, 9]], [2, "Second", [7, 9]]]);
    return true;
  });
  assert.throws(() => translate(text, plan, {toolTable: "T7 P7\n"}), e => e.context.operations.every(x => x.missing.length === 1 && x.missing[0].role === "H-offset"));
});

test("malformed or ambiguous tool tables fail with the source line", () => {
  const {text, plan} = example();
  for (const [table, code] of [
    ["T1 P1\nT1 P2", "TOOL_TABLE_DUPLICATE"], ["T1 P1\nT2", "TOOL_TABLE_SYNTAX"],
    ["T1 P1\nT2 P2 Q10", "TOOL_TABLE_SYNTAX"], ["T1 P1\nT2 P2 XNaN", "TOOL_TABLE_SYNTAX"],
    ["T1 P1\nT2 P2 X1 X2", "TOOL_TABLE_SYNTAX"], ["T1 P1\nT2 P2 R.2", "TOOL_TABLE_SYNTAX"],
    ["T1 P1\nT2 P-1", "TOOL_TABLE_SYNTAX"], ["T1 P1\nT2.5 P2", "TOOL_TABLE_SYNTAX"],
    ["T1 P1\nT2 P2 Z1e999", "TOOL_TABLE_SYNTAX"], ["T1 P1\nT2e1 P2", "TOOL_TABLE_SYNTAX"],
    ["T1 P1\nT2 P2.0", "TOOL_TABLE_SYNTAX"], ["T1 P1\nT2 P2 Q1e0", "TOOL_TABLE_SYNTAX"]
  ]) assert.throws(() => translate(text, plan, {toolTable: table}), e => e.code === code && e.context.toolTableLine === 2, table);
});

test("source map locates every exact motion, full circle, dwell and transition in both units", () => {
  for (const fixture of [example, continuationExample, linkExample]) for (const units of ["mm", "inch"]) {
    const {text, plan} = fixture(units), out = translate(text, plan), model = readProgram(text).model;
    const lines = out.gcode.trimEnd().split("\n");
    assert.deepEqual(out.sourceMap.map(x => x.line), lines.map((_, i) => i + 1));
    assert.equal(out.report.traceability.gcodeSHA256, crypto.createHash("sha256").update(out.gcode).digest("hex"));
    for (const item of out.sourceMap) {
      const line = lines[item.line - 1];
      if (item.action === "motion") {
        assert.match(line, /^(?:G53 )?G[0123] /);
        if (item.phase === "toolpath") {
          const p = model.sections[item.section - 1].paths[item.path - 1];
          if (p.kind === "arc") {
            assert.deepEqual(item.motion.sourceCenter, p.center); assert.equal(item.motion.clockwise, p.clockwise);
            assert.deepEqual(item.motion.centerOffset, [p.center[0] - p.start[0], 0, p.center[2] - p.start[2]]);
            assert.deepEqual(item.motion.end, p.fullCircle ? p.start : p.end);
          } else {
            assert.deepEqual(item.motion.start, p.points[item.fromVertex - 1]);
            assert.deepEqual(item.motion.end, p.points[item.toVertex - 1]);
          }
        } else {
          assert.equal(item.motion.frame, line.startsWith("G53") ? "machine" : "work");
          assert.equal(Object.hasOwn(item.motion, "start"), false, "Unknown machine position must not be invented");
          assert.equal(Object.keys(item.motion.target).length, 1);
        }
      } else if (item.action === "dwell") assert.equal(line, "G4 P" + item.seconds);
      else assert.doesNotMatch(line, /^(?:G53 )?G[01234] /);
    }
    for (const range of out.report.traceability.operationRanges) {
      assert.match(lines[range.firstLine - 1], new RegExp(`Section ${range.section}:`));
      assert.equal(out.sourceMap[range.lastLine - 1].section, range.section);
      assert.equal(out.sourceMap[range.lastLine].section === range.section, false);
    }
    const links = out.sourceMap.filter(x => x.phase === "link" && x.action === "motion");
    if (fixture === linkExample) assert.deepEqual(links.map(x => x.waypoint), [1, 2, 3]);
  }
});

test("compacted polylines retain individually traceable segments and repeated vertices", () => {
  const p = new Program({units: "mm"}), s = p.addSection({name: "Zigzag", tool: {number: 1, offset: 1}, workOffset: 1, start: [2, 0, 0], spindle: {mode: "rpm", speed: 600, clockwise: true}, coolant: "off"});
  for (const end of [[3, 0, 0], [4, 0, -1], [3, 0, 0]]) s.linear(end, {mode: "perMinute", value: 100});
  const text = p.toSTEP(), out = translate(text, simulationPlan(readProgram(text)));
  const body = out.sourceMap.filter(x => x.phase === "toolpath" && x.action === "motion");
  assert.deepEqual(body.map(x => [x.path, x.segment, x.fromVertex, x.toVertex]), [[1, 1, 1, 2], [1, 2, 2, 3], [1, 3, 3, 4]]);
  assert.deepEqual(body[2].motion.end, [3, 0, 0]);
});

test("CLI preflight produces JSON only, shares translation checks and preserves all inputs", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-preflight-"));
  try {
    const input = path.join(dir, "input.stpnc"), planPath = path.join(dir, "plan.json"), table = path.join(dir, "tool.tbl"), output = path.join(dir, "review.json"), diag = path.join(dir, "diagnostics");
    const {text, plan} = example(); fs.writeFileSync(input, text); fs.writeFileSync(planPath, JSON.stringify(plan)); fs.writeFileSync(table, "T1 P1\n");
    const run = (...args) => spawnSync(process.execPath, [path.join(__dirname, "../bin/nextnc.js"), ...args], {encoding: "utf8", env: {...process.env, NEXTNC_DIAGNOSTICS: diag}});
    const args = [input, "--plan", planPath, "--tool-table", table];
    let r = run("preflight", ...args); assert.equal(r.status, 0, r.stderr);
    const review = JSON.parse(r.stdout); assert.equal(review.gcodeWritten, false); assert.equal(review.inspection.toolTable.status, "passed");
    const archive = JSON.parse(fs.readFileSync(path.join(diag, "latest.json")));
    assert.equal(archive.toolTable, table); assert.equal(archive.toolTableSHA256.length, 64);
    r = run("preflight", ...args, "--output", output); assert.equal(r.status, 0, r.stderr); assert.equal(r.stdout, "");
    const preserved = fs.readFileSync(output, "utf8"); assert.equal(run("preflight", ...args, "--output", output).status, 1); assert.equal(fs.readFileSync(output, "utf8"), preserved);
    r = run("preflight", input, "--plan", planPath); assert.equal(JSON.parse(r.stdout).inspection.toolTable.status, "not_checked");
    fs.writeFileSync(table, "T9 P9\n");
    for (const command of ["preflight", "translate"]) {
      const failedOutput = path.join(dir, command + ".new");
      r = run(command, ...args, "--output", failedOutput); assert.equal(r.status, 1); assert.equal(r.stdout, ""); assert.match(r.stderr, /TOOL_TABLE_MISSING/); assert.equal(fs.existsSync(failedOutput), false);
    }
    assert.equal(run("preflight", input).status, 1);
    for (const command of ["inspect", "plan-template"]) assert.equal(run(command, input, "--tool-table", table).status, 2);
    assert.equal(fs.readFileSync(input, "utf8"), text); assert.equal(fs.readFileSync(planPath, "utf8"), JSON.stringify(plan)); assert.equal(fs.readFileSync(table, "utf8"), "T9 P9\n");
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
