"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const {spawnSync} = require("node:child_process");
const {LinuxCNCOutput} = require("../src/linuxcnc-output"), {translate} = require("../src/translate");
const {auditGcode} = require("../src/gcode-audit"), {semanticFixture} = require("./support/semantic-fixture");
const {example, millingExample} = require("../scripts/example"), {renderReport} = require("../src/report");
const faults = [
  ["arc sense", c => c.type === "arc", s => s.replace(/^G[23]/, x => x === "G2" ? "G3" : "G2")],
  ["fractional feed", c => c.type === "feed" && c.mode === "perRevolution", s => s.replace("F0.2", "F0.3")],
  ["machine frame", c => c.type === "rapid" && c.frame === "machine", s => s.replace("G53 ", "")],
  ["spindle direction", c => c.type === "spindleStart", s => s.replace(/^M[34]/, x => x === "M3" ? "M4" : "M3")],
  ["missing end", c => c.type === "end", () => "(Missing program end)"],
  ["axis", c => c.type === "linear", s => s.replace(/X[-.\d]+/, "X999")],
  ["centre", c => c.type === "arc", s => s.replace(/[IJK][-.\d]+/, x => x[0] + "999")],
  ["units", c => c.type === "initialize", s => s.replace(/^G2[01]/, x => x === "G21" ? "G20" : "G21")],
  ["plane", c => c.type === "initialize", s => s.replace(/G1[789]/, x => x === "G18" ? "G17" : "G18")],
  ["diameter mode", c => c.type === "initialize", s => s.replace("G8", "G7")],
  ["arc distance mode", c => c.type === "initialize", s => s.replace("G91.1", "G90.1")],
  ["distance mode", c => c.type === "initialize", s => s.replace(" G90 ", " G91 ")],
  ["path control", c => c.type === "initialize", s => s.replace("G61", "G64")],
  ["duplicate G", c => c.type === "initialize", s => s + " G8"],
  ["temporary offsets", c => c.type === "clearTemporaryOffsets", () => "G92.2"],
  ["tool", c => c.type === "toolChange", s => s.replace(/T\d+/, "T99")],
  ["H offset", c => c.type === "toolOffset", s => s.replace(/H\d+/, "H99")],
  ["work offset", c => c.type === "workOffset", () => "G59.3"],
  ["coolant", c => c.type === "coolant" && c.value === "mist", () => "M8"],
  ["dwell", c => c.type === "dwell", () => "G4 P0.5"],
  ["rapid to feed", c => c.type === "rapid", s => s.replace("G0", "G1")],
  ["wrong work frame", c => c.type === "rapid" && c.frame === "work", s => "G53 " + s],
  ["spindle selection", c => c.type === "spindleStart", s => s.replace("$0", "$1")],
  ["RPM value", c => c.type === "spindleMode" && c.speed === 700, s => s.replace("700", "701")],
  ["exponent", c => c.type === "feed" && c.mode === "perRevolution", s => s.replace("0.2", "2e-1")],
  ["comma", c => c.type === "feed" && c.mode === "perRevolution", s => s.replace("0.2", "0,2")],
  ["NaN", c => c.type === "feed" && c.mode === "perRevolution", s => s.replace("0.2", "NaN")],
  ["duplicate axis", c => c.type === "linear", s => s + " X3"],
  ["extra command", c => c.type === "linear", s => s + " M3"],
  ["embedded newline", c => c.type === "linear", s => s + "\nM2"],
  ["comment injection", c => c.type === "comment", () => "(DEBUG, injected)"],
  ["mode-only shutdown feed", c => c.type === "feed" && c.value === undefined, () => "G95"],
  ["cancel offset", c => c.type === "cancelToolOffset", () => "G43 H1"]
];
function corrupt(fixture, match, change) {
  const original = LinuxCNCOutput.prototype.emit; let changed;
  try {
    LinuxCNCOutput.prototype.emit = function(c, s, d) {
      original.call(this, c, s, d);
      if (!changed && match(c)) {
        const i = this.lines.length - 1, before = this.lines[i], after = change(before), records = JSON.stringify(this.sourceMap);
        assert.notEqual(before, after); this.lines[i] = after;
        assert.equal(JSON.stringify(this.sourceMap), records); changed = {line: i + 1, after, command: c};
      }
    };
    assert.throws(() => translate(fixture.text, fixture.plan), e => {
      assert.ok(changed, "fault predicate must match");
      assert.equal(e.code, "INTERNAL_ERROR"); assert.match(e.context.invariant, /^SERIALIZATION_/);
      const inserted = changed.after.includes("\n");
      assert.equal(e.context.line, changed.line + (inserted ? 1 : 0)); assert.equal(e.context.observedBlock, changed.after.split("\n")[inserted ? 1 : 0]);
      if (!inserted) assert.equal(e.context.expectedCommand.type, changed.command.type);
      assert.ok(e.context.provenance);
      return true;
    });
  } finally { LinuxCNCOutput.prototype.emit = original; }
}
for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
  test(`${machine} ${units}: final text faults are rejected with unchanged command records`, () => {
    const f = semanticFixture(machine, units);
    for (const [name, match, change] of faults) {
      try { corrupt(f, match, change); } catch (error) { error.message = name + ": " + error.message; throw error; }
    }
    if (machine === "lathe") {
      corrupt(f, c => c.type === "spindleMode" && c.mode === "css", s => s.replace(/ D\d+/, ""));
      corrupt(f, c => c.type === "spindleMode" && c.mode === "css", s => s.replace("G96", "G97"));
    }
    const circles = (machine === "lathe" ? example : millingExample)(units);
    corrupt(circles, c => c.type === "arc" && c.fullCircle, s => s.replace(" P1", ""));
    corrupt(circles, c => c.type === "arc" && c.fullCircle, s => s + " X0");
    const out = translate(f.text, f.plan), audit = out.report.execution.serialization;
    assert.equal(audit.status, "passed"); assert.equal(audit.checkedLines, out.sourceMap.length);
    assert.equal(audit.executableBlocks + audit.comments, audit.checkedLines);
    assert.deepEqual(auditGcode(out.gcode, out.sourceMap, {machine, units}), audit);
    assert.throws(() => auditGcode(out.gcode + "M2\n", out.sourceMap, {machine, units}), e => e.context.invariant === "SERIALIZATION_ASSOCIATION");
    assert.throws(() => auditGcode(out.gcode.slice(0, -1), out.sourceMap, {machine, units}), e => e.context.invariant === "SERIALIZATION_TEXT");
  });
}
test("comment audit requires passive prefixes even when command and text agree on a directive", () => {
  const f = semanticFixture("lathe", "mm"), out = translate(f.text, f.plan);
  for (const text of ["DEBUG, data", "PROBEOPEN filename", "PROBECLOSE", "py, command", "m s g, message"]) {
    const map = structuredClone(out.sourceMap); map[0].command.text = text;
    const gcode = `(${text})\n` + out.gcode.slice(out.gcode.indexOf("\n") + 1);
    assert.throws(() => auditGcode(gcode, map, {machine: "lathe", units: "mm"}), e => e.context.invariant === "SERIALIZATION_COMMENT_PREFIX");
  }
});
test("serialization reports retain older archive uncertainty and safely render details", () => {
  const f = semanticFixture("mill", "mm"), out = translate(f.text, f.plan);
  const record = {schema: "linuxcnc-next-nc/diagnostic/1", inspection: out.report};
  assert.match(renderReport(record), /Final G-code serialization audit/);
  assert.match(renderReport(record), /serialization-audit\/1/);
  delete record.inspection.execution.serialization;
  assert.match(renderReport(record), /Final G-code serialization audit<\/h2><p>Not recorded/);
});
test("text corruption leaves CLI output empty, preserves existing destinations and archives provenance", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-text-audit-"));
  try {
    const preload = path.join(dir, "fault.cjs"), input = path.join(dir, "input.stpnc"), plan = path.join(dir, "plan.json"), output = path.join(dir, "result.ngc"), diagnostics = path.join(dir, "diagnostics");
    fs.writeFileSync(preload, `const {LinuxCNCOutput}=require(${JSON.stringify(require.resolve("../src/linuxcnc-output"))});const original=LinuxCNCOutput.prototype.emit;LinuxCNCOutput.prototype.emit=function(c,s,d){original.call(this,c,s,d);if(c.type==='arc')this.lines[this.lines.length-1]+=' M3';};`);
    for (const machine of ["lathe", "mill"]) for (const command of ["preflight", "translate", "filter"]) {
      const f = semanticFixture(machine, "mm"); fs.writeFileSync(input, f.text); fs.writeFileSync(plan, JSON.stringify(f.plan));
      const args = ["--require", preload, path.join(__dirname, "../bin/nextnc.js"), command, input, ...(command === "filter" ? [] : ["--plan", plan])];
      const run = more => spawnSync(process.execPath, [...args, ...more], {encoding: "utf8", env: {...process.env, NEXTNC_PLAN: plan, NEXTNC_TOOL_TABLE: "", NEXTNC_DIAGNOSTICS: diagnostics}});
      const r = run([]); assert.equal(r.status, 1); assert.equal(r.stdout, "");
      const archive = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json"))), error = archive.error;
      assert.equal(archive.inspection.validationCoverage.stages.completeness.status, "passed");
      assert.equal(archive.inspection.validationCoverage.stages.policy.status, "passed");
      assert.equal(archive.inspection.validationCoverage.stages.serialization.status, "failed");
      assert.equal(archive.inspection.validationCoverage.stages.globalRules.status, "not_checked");
      assert.equal(error.code, "INTERNAL_ERROR"); assert.equal(error.context.invariant, "SERIALIZATION_WORDS");
      assert.equal(error.context.operation, "First"); assert.ok(error.context.provenance.curve.record);
      if (command !== "filter") {
        assert.equal(run(["--output", output]).status, 1); assert.ok(!fs.existsSync(output));
        fs.writeFileSync(output, "keep existing"); assert.equal(run(["--output", output]).status, 1); assert.equal(fs.readFileSync(output, "utf8"), "keep existing"); fs.unlinkSync(output);
      }
      assert.equal(fs.readFileSync(input, "utf8"), f.text); assert.deepEqual(JSON.parse(fs.readFileSync(plan)), f.plan);
    }
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
