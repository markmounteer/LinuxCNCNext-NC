"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), fs = require("node:fs"), path = require("node:path"), os = require("node:os");
const {spawnSync} = require("node:child_process");
const {LinuxCNCOutput} = require("../src/linuxcnc-output"), {InternalError} = require("../src/internal-error");
const {translate} = require("../src/translate"), {semanticFixture} = require("./support/semantic-fixture");
const {example, millingExample} = require("../scripts/example");
const {renderReport} = require("../src/report");
const init = (machine, units) => ({type: "initialize", units, plane: machine === "mill" ? "XY" : "XZ"});
test("command contracts reject malformed values and lifecycle errors atomically for both machines/units", () => {
  for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
    const out = new LinuxCNCOutput({machine, units}), source = {phase: "test", section: 1, path: 1};
    function fails(c, rule) {
      const before = JSON.stringify(out);
      assert.throws(() => out.emit(c, source), e => e instanceof InternalError && e.code === "INTERNAL_ERROR" && e.context.invariant === rule && e.context.section === 1);
      assert.equal(JSON.stringify(out), before);
    }
    fails({type: "linear", frame: "work", axes: {x: 1}}, "COMMAND_BEFORE_INIT");
    out.emit(init(machine, units), source);
    const plane = machine === "mill" ? "XY" : "XZ", arcAxes = machine === "mill" ? {x: 1, y: 0} : {x: 1, z: 0};
    const arc = {type: "arc", frame: "work", plane, axes: arcAxes, centerOffset: arcAxes, clockwise: false, fullCircle: false};
    for (const [c, rule] of [
      [{type: "unknown"}, "COMMAND_TYPE"], [{type: "end", extra: true}, "COMMAND_FIELDS"],
      [{type: "linear", frame: "work", axes: {a: 1}}, "MOTION_AXES"],
      [{type: "linear", frame: "work", axes: {}}, "MOTION_AXES"],
      [{type: "linear", frame: "work", axes: {x: NaN}}, "MOTION_AXES"],
      [{type: "rapid", frame: "unknown", axes: {x: 1}}, "MOTION_FRAME"],
      [{...init(machine, units), units: "metres"}, "COMMAND_UNITS"],
      [{...init(machine, units), units: units === "mm" ? "inch" : "mm"}, "COMMAND_UNITS"],
      [{type: "toolChange", tool: 0}, "TOOL_ID"], [{type: "toolOffset", offset: "1"}, "TOOL_ID"],
      [{type: "workOffset", value: "G54\nG0 X20"}, "WORK_OFFSET"],
      [{type: "feed", mode: "inverse", value: 20}, "FEED_VALUE"],
      [{type: "coolant", value: "through"}, "COOLANT_VALUE"],
      [{type: "dwell", frame: "work", seconds: 0}, "DWELL_VALUE"],
      [{type: "spindleStart", clockwise: 1}, "SPINDLE_DIRECTION"],
      [{...arc, clockwise: 1}, "ARC_FLAGS"], [{...arc, plane: plane === "XY" ? "XZ" : "XY"}, "ARC_PLANE"],
      [{...arc, frame: "machine"}, "MOTION_FRAME"], [{...arc, centerOffset: {x: 1}}, "ARC_CENTER"],
      [{...arc, fullCircle: true}, "ARC_ENDPOINT"]
    ]) fails(c, rule);
    if (machine === "lathe") fails({type: "rapid", frame: "work", axes: {y: 1}}, "MOTION_AXES");
    else fails({type: "spindleMode", mode: "css", speed: 80, maximumRPM: 2000}, "SPINDLE_MODE");
    out.emit({...arc, axes: {}, fullCircle: true}, source);
    out.emit({type: "toolChange", tool: 2}, source);
    fails({type: "rapid", frame: "work", axes: {x: 1}}, "MOTION_STATE");
    out.emit(init(machine, units), source); out.emit({type: "rapid", frame: "machine", axes: {x: 1}}, source);
    out.emit({type: "end"}, source);
    fails({type: "rapid", frame: "work", axes: {x: 1}}, "COMMAND_AFTER_END");
    fails({type: "end"}, "COMMAND_AFTER_END");
  }
});
test("successful source maps bind STEP uses and plan pointers without changing G-code after renumbering", () => {
  for (const factory of [example, millingExample]) for (const units of ["mm", "inch"]) {
    const f = factory(units), a = translate(f.text, f.plan);
    const changed = f.text.replace(/#(\d+)/g, (_, n) => "#" + (Number(n) + 10000)).replace(/\n#/g, "\n  #").replace(/\n/g, "\r\n");
    const b = translate(changed, f.plan);
    assert.equal(a.gcode, b.gcode); assert.deepEqual(a.report.programFingerprint, b.report.programFingerprint);
    assert.notEqual(a.report.traceability.provenance.inputSHA256, b.report.traceability.provenance.inputSHA256);
    for (const e of b.sourceMap) {
      assert.ok(e.provenance);
      if (e.provenance.origin === "step") {
        for (const key of ["workingstep", "operation", "toolpath", "curve", "from", "to"]) {
          const ref = e.provenance[key];
          if (ref) assert.ok(changed.split(/\r?\n/)[ref.sourceLine - 1].slice(ref.sourceColumn - 1).startsWith(ref.record + "="));
        }
      } else if (e.provenance.origin === "execution-plan") {
        const value = e.provenance.pointer.split("/").slice(1).reduce((v, k) => v[k], f.plan);
        if (e.action === "motion") assert.deepEqual(value, e.command.axes);
        else assert.equal(value, e.command.tool ?? e.command.offset ?? e.command.value);
      }
    }
    assert.equal(a.report.execution.completeness.status, "passed");
    assert.equal(a.report.execution.completeness.checkedUses, a.sourceMap.filter(e => ["motion", "dwell"].includes(e.action)).length);
    const html = renderReport({schema: "linuxcnc-next-nc/diagnostic/1", inspection: b.report, sourceMap: b.sourceMap});
    assert.match(html, /sourceColumn/); assert.match(html, /completeness/);
  }
});
test("ordered audit detects dropped, duplicated, reordered and corrupted uses for both machines/units", () => {
  const original = LinuxCNCOutput.prototype.emit;
  const cases = [
    ["drop", (c, s, d) => s.phase === "toolpath" && d.action === "motion", "USE_ORDER"],
    ["duplicate", (c, s, d) => s.phase === "toolpath" && d.action === "motion", "USE_ORDER"],
    ["drop", (c) => c.type === "dwell", "USE_ORDER"],
    ["drop", (c, s, d) => d.stage === "link", "WAYPOINT_USE"],
    ["duplicate", (c, s, d) => d.stage === "program-end", "WAYPOINT_USE"]
  ];
  try {
    for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
      const f = semanticFixture(machine, units);
      for (const [kind, match, rule] of cases) {
        let injected = false;
        LinuxCNCOutput.prototype.emit = function(c, s, d) {
          if (!injected && match(c, s, d)) { injected = true; if (kind === "drop") return; original.call(this, c, s, d); }
          return original.call(this, c, s, d);
        };
        assert.throws(() => translate(f.text, f.plan), e => e.code === "INTERNAL_ERROR" && e.context.invariant === rule, `${machine}/${units} ${kind}`);
        assert.ok(injected);
      }
      for (const [mutate, rule] of [
        [e => {e.path += 1;}, "USE_ORDER"],
        [e => {e.modalState.spindleSpeed += 1;}, "PROCESS_STATE"],
        [e => {e.motion.end[0] += 1;}, "MOTION_DETAIL"],
        [e => {e.command.centerOffset.x += 1;}, "ARC_USE"],
        [e => {e.provenance.operation.record = "#999";}, "SOURCE_PROVENANCE"],
        [e => {e.line += 1;}, "MAP_LINE"],
        [e => {e.action = "state";}, "MAP_ACTION"]
      ]) {
        LinuxCNCOutput.prototype.emit = function(c, s, d) {
          original.call(this, c, s, d);
          if (c.type === "end") mutate(this.sourceMap.find(e => e.command.type === "arc"));
        };
        assert.throws(() => translate(f.text, f.plan), e => e.code === "INTERNAL_ERROR" && e.context.invariant === rule, rule);
      }
      LinuxCNCOutput.prototype.emit = function(c, s, d) {
        original.call(this, c, s, d);
        if (c.type === "end") {
          const ids = this.sourceMap.flatMap((e, i) => e.stage === "link" ? [i] : []), [a, b] = ids;
          [this.sourceMap[a], this.sourceMap[b]] = [this.sourceMap[b], this.sourceMap[a]];
          this.sourceMap[a].line = a + 1; this.sourceMap[b].line = b + 1;
        }
      };
      assert.throws(() => translate(f.text, f.plan), e => e.context?.invariant === "WAYPOINT_USE");
    }
  } finally { LinuxCNCOutput.prototype.emit = original; }
});
test("audit failures publish no G-code or preflight result and retain structured CLI diagnostics", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-audit-"));
  try {
    const preload = path.join(dir, "fault.cjs"), input = path.join(dir, "input.stpnc"), planFile = path.join(dir, "plan.json"), output = path.join(dir, "result.ngc"), diagnostics = path.join(dir, "diagnostics");
    fs.writeFileSync(preload, `const {LinuxCNCOutput}=require(${JSON.stringify(require.resolve("../src/linuxcnc-output"))});const original=LinuxCNCOutput.prototype.emit;let dropped=false;LinuxCNCOutput.prototype.emit=function(c,s,d){if(!dropped&&s.phase==='toolpath'&&d.action==='motion'){dropped=true;return;}return original.call(this,c,s,d);};`);
    for (const machine of ["lathe", "mill"]) for (const command of ["preflight", "translate", "filter"]) {
      const f = semanticFixture(machine, "mm"); fs.writeFileSync(input, f.text); fs.writeFileSync(planFile, JSON.stringify(f.plan));
      const args = ["--require", preload, path.join(__dirname, "../bin/nextnc.js"), command, input];
      if (command !== "filter") args.push("--plan", planFile);
      if (command === "translate") args.push("--output", output);
      const r = spawnSync(process.execPath, args, {encoding: "utf8", env: {...process.env, NEXTNC_PLAN: planFile, NEXTNC_DIAGNOSTICS: diagnostics}});
      assert.equal(r.status, 1); assert.equal(r.stdout, ""); assert.ok(!fs.existsSync(output));
      const record = JSON.parse(fs.readFileSync(path.join(diagnostics, "latest-error.json")));
      assert.equal(record.error.code, "INTERNAL_ERROR"); assert.equal(record.error.context.invariant, "USE_ORDER");
      assert.ok(record.error.context.expectedProvenance.operation.record);
      assert.equal(record.inputSHA256, require("node:crypto").createHash("sha256").update(f.text).digest("hex"));
    }
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
