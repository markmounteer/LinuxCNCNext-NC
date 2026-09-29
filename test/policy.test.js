"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const fs = require("node:fs"), os = require("node:os"), path = require("node:path"), {spawnSync} = require("node:child_process");
const {LinuxCNCOutput} = require("../src/linuxcnc-output"), {translate} = require("../src/translate");
const {semanticFixture} = require("./support/semantic-fixture"), {processStateFixture} = require("./support/process-state-fixture");
const {renderReport} = require("../src/report");
const pairs = [["lathe", "mm"], ["lathe", "inch"], ["mill", "mm"], ["mill", "inch"]];
const original = LinuxCNCOutput.prototype.emit;
function fault(fixture, wrapper, rule) {
  try {
    LinuxCNCOutput.prototype.emit = wrapper;
    assert.throws(() => translate(fixture.text, fixture.plan), e => {
      assert.equal(e.code, "INTERNAL_ERROR"); assert.equal(e.context.invariant, "POLICY_" + rule);
      assert.equal(e.validationCoverage.stages.completeness.status, "passed");
      assert.equal(e.validationCoverage.stages.policy.status, "failed");
      assert.equal(e.validationCoverage.stages.serialization.status, "not_checked");
      assert.ok(e.context.line && e.context.boundary && e.context.expected && e.context.observed);
      return true;
    });
  } finally { LinuxCNCOutput.prototype.emit = original; }
}
for (const [machine, units] of pairs) test(`${machine} ${units}: process state timing, M6 resets and independent policy counts`, () => {
  for (const coolant of ["off", "flood", "mist"]) {
    const f = processStateFixture(machine, units, coolant), result = translate(f.text, f.plan);
    assert.equal(result.gcode.split("\n").filter(l => !l.startsWith("(")).join("\n"), f.reference);
    assert.equal(result.report.validationCoverage.stages.policy.status, "passed");
    const counts = result.report.execution.policy.checked;
    assert.deepEqual([counts.startup, counts.toolChanges, counts.operationEntries, counts.reversals, counts.programEnd], [1,2,3,1,1]);
    for (const entry of result.sourceMap.filter(e => e.stage === "approach")) {
      assert.equal(entry.modalState.spindleDirection, "stopped"); assert.equal(entry.modalState.coolant, "off");
      assert.equal(entry.modalState.spindleMode, "rpm"); assert.equal(entry.modalState.spindleSpeed, 0);
    }
    for (const entry of result.sourceMap.filter(e => e.command.type === "toolChange")) {
      assert.equal(entry.stateChange.coolant.after, null);
      assert.deepEqual(result.sourceMap.slice(entry.line, entry.line + 4).map(e => e.command.type), ["initialize", "spindleStop", "coolant", "spindleMode"]);
    }
    assert.ok(!result.sourceMap.some(e => e.section === 3 && e.command.type === "toolChange"));
    const equalFeed = result.sourceMap.filter(e => e.section === 1 && e.command.type === "feed");
    assert.deepEqual(equalFeed.map(e => [e.command.mode, e.command.value]), [["perMinute",0.2],["perRevolution",0.2]]);
    if (machine === "lathe") assert.deepEqual(result.sourceMap.filter(e => e.command.mode === "css").map(e => [e.command.speed,e.command.maximumRPM]), [[600,1800],[600,1900]]);
  }
  const f = semanticFixture(machine, units), result = translate(f.text, f.plan);
  const counts = result.report.execution.policy.checked;
  assert.deepEqual([counts.continuations, counts.linkWaypoints, counts.toolChanges], [1,machine === "mill" ? 3 : 2,2]);
  assert.ok(!result.sourceMap.some(e => [2,3].includes(e.section) && e.phase !== "toolpath" && e.command.type === "spindleStop"));
});
test("all 16 omissions from the review fail policy even when command and text are both absent", () => {
  const cases = [
    [(c,s) => s.phase === "program-end" && c.type === "spindleStop", "END_RETRACT_STATE"],
    [(c,s) => s.phase === "program-end" && c.type === "coolant", "END_RETRACT_STATE"],
    [(c,s) => s.phase === "transition" && s.section === 4 && c.type === "spindleStop", "RETRACT_STATE"],
    [(c,s) => s.phase === "toolpath" && c.type === "spindleStop", "REVERSAL_STOP"]
  ];
  let rejected = 0;
  for (const pair of pairs) for (const [match, rule] of cases) {
    let dropped = 0;
    fault(semanticFixture(...pair), function(c,s,d) { if (!dropped && match(c,s)) { ++dropped; return; } original.call(this,c,s,d); }, rule);
    assert.equal(dropped, 1); ++rejected;
  }
  assert.equal(rejected, 16);
});
test("startup, post-M6, shutdown, initial operation state and mapping omissions fail independently", () => {
  for (const pair of pairs) {
    const f = processStateFixture(...pair, "flood");
    const cases = [
      [(c,s) => s.phase === "header" && c.type === "spindleStop", "STARTUP"],
      [(c,s) => s.phase === "header" && c.type === "coolant", "STARTUP"],
      [(c,s) => s.phase === "header" && c.type === "spindleMode", "STARTUP"],
      [(c,s) => s.phase === "transition" && s.section === 1 && c.type === "spindleStop", "MAPPING_STATE"],
      [(c,s) => s.phase === "transition" && s.section === 1 && c.type === "coolant", "MAPPING_STATE"],
      [(c,s) => s.phase === "transition" && s.section === 1 && c.type === "spindleMode" && c.speed === 0, "MAPPING_STATE"],
      [(c,s) => s.phase === "program-end" && c.type === "cancelToolOffset", "END_FEED_ORDER"],
      [(c,s) => s.phase === "program-end" && c.type === "spindleMode", "END_RETRACT_STATE"]
    ];
    for (const [match,rule] of cases) {
      let dropped = 0;
      fault(f, function(c,s,d) { if (!dropped && match(c,s)) { ++dropped; return; } original.call(this,c,s,d); }, rule);
      assert.equal(dropped,1);
    }
    let droppedInitial = false;
    fault(f, function(c,s,d) {
      original.call(this,c,s,d);
      if (!droppedInitial && s.section === 1 && s.phase === "transition" && c.type === "spindleMode" && c.speed === 500) {
        this.lines.pop(); this.sourceMap.pop(); droppedInitial = true;
      }
    }, "OPERATION_ENTRY");
    assert.ok(droppedInitial);
  }
});
test("late resets, wrong-boundary stops, early restarts and premature offset cancellation are rejected", () => {
  for (const pair of pairs) {
    const f = semanticFixture(...pair);
    // Move the actual stop AND its line past the first final retract.
    let held;
    fault(f, function(c,s,d) {
      if (s.phase === "program-end" && c.type === "spindleStop") { held = [c,s,d]; return; }
      original.call(this,c,s,d);
      if (held && d.stage === "program-end") { original.call(this,...held); held = null; }
    }, "END_RETRACT_STATE");
    assert.equal(held,null);
    let inserted = false;
    fault(f, function(c,s,d) {
      if (!inserted && s.phase === "program-end" && c.type === "rapid") {
        inserted = true; original.call(this,{type:"spindleMode",mode:"rpm",speed:900},s); original.call(this,{type:"spindleStart",clockwise:true},s);
      }
      original.call(this,c,s,d);
    }, "END_RETRACT_STATE");
    assert.ok(inserted);
    // End offset cancellation belongs after all reviewed G53 waypoints.
    inserted = false;
    fault(f, function(c,s,d) {
      if (!inserted && s.phase === "program-end" && c.type === "rapid") { inserted = true; original.call(this,{type:"cancelToolOffset"},s); }
      original.call(this,c,s,d);
    }, "RESET_ORDER");
    // A stop in a continue boundary cannot stand in for a later retract stop.
    fault(f, function(c,s,d) {
      original.call(this,c,s,d);
      if (s.section === 2 && c.type === "comment") {
        original.call(this,{type:"spindleStop"},s);
        original.call(this,{type:"spindleStart",clockwise:true},s);
      }
    }, "CONTINUITY");
  }
});
test("the auditor does not trust fabricated emitter snapshots or state changes", () => {
  for (const pair of pairs) {
    const f = semanticFixture(...pair);
    let dropped = false;
    fault(f, function(c,s,d) {
      original.call(this,c,s,d);
      if (!dropped && s.phase === "toolpath" && c.type === "feed" && c.mode === "perRevolution") {
        // Keep the emitter's state as if F/G95 happened; remove both records.
        this.lines.pop(); this.sourceMap.pop(); dropped = true;
      }
    }, "PATH_STATE");
    assert.ok(dropped);
  }
});
test("first and later M6 and every restoration command remain required despite misleading emitter state", () => {
  for (const pair of pairs) for (const section of [1,2]) for (const type of ["toolChange","spindleStop","coolant","spindleMode"]) {
    const f = processStateFixture(...pair,"off"); let afterChange = false, dropped = 0;
    fault(f, function(c,s,d) {
      original.call(this,c,s,d);
      if (s.section === section && c.type === "toolChange") afterChange = true;
      if (!dropped && afterChange && s.section === section && c.type === type) {
        this.lines.pop(); this.sourceMap.pop(); ++dropped;
      }
    }, "MAPPING_STATE");
    assert.equal(dropped,1);
  }
});
test("known inherited state satisfies policy without a redundant instruction", () => {
  for (const pair of pairs) {
    const f = processStateFixture(...pair,"off"); let dropped = 0;
    try {
      LinuxCNCOutput.prototype.emit = function(c,s,d) {
        // Initial CW is already running; the first path only increases RPM.
        if (s.section === 1 && s.phase === "toolpath" && c.type === "spindleStart" && c.clockwise) { ++dropped; return; }
        original.call(this,c,s,d);
      };
      assert.equal(translate(f.text,f.plan).report.execution.policy.status,"passed");
      assert.equal(dropped,1);
    } finally { LinuxCNCOutput.prototype.emit = original; }
  }
});
test("JSON/HTML policy coverage preserves old archives and escapes diagnostics", () => {
  const f = semanticFixture("mill","mm"), result = translate(f.text,f.plan);
  const archive = {schema:"linuxcnc-next-nc/diagnostic/1", inspection:result.report};
  assert.match(renderReport(archive), /policy-audit\/1/);
  delete archive.inspection.execution.policy; delete archive.inspection.validationCoverage.stages.policy;
  const before = JSON.stringify(archive);
  assert.match(renderReport(archive), /Translator policy audit<\/h2><p>Not recorded/);
  assert.equal(JSON.stringify(archive), before);
  archive.error = {code:"INTERNAL_ERROR",context:{invariant:"POLICY_RETRACT_STATE",operation:"<script>bad</script>",expected:{coolant:"off"},observed:{coolant:"flood"}}};
  assert.doesNotMatch(renderReport(archive), /<script>/); assert.match(renderReport(archive), /&lt;script&gt;/);
});
test("policy failures publish no partial files and retain stage and boundary diagnostics", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(),"nextnc-policy-"));
  try {
    const preload=path.join(dir,"fault.cjs"), input=path.join(dir,"input.stpnc"), plan=path.join(dir,"plan.json"), output=path.join(dir,"output.ngc"), diagnostics=path.join(dir,"diagnostics");
    fs.writeFileSync(preload, `const {LinuxCNCOutput}=require(${JSON.stringify(require.resolve("../src/linuxcnc-output"))});const original=LinuxCNCOutput.prototype.emit;LinuxCNCOutput.prototype.emit=function(c,s,d){if(s.phase==='program-end'&&c.type==='spindleStop')return;original.call(this,c,s,d);};`);
    for (const machine of ["lathe","mill"]) for (const command of ["preflight","translate","filter"]) {
      const f = semanticFixture(machine,"mm"); fs.writeFileSync(input,f.text); fs.writeFileSync(plan,JSON.stringify(f.plan));
      const args = ["--require",preload,path.join(__dirname,"../bin/nextnc.js"),command,input,...(command === "filter" ? [] : ["--plan",plan])];
      const run = more => spawnSync(process.execPath,[...args,...more],{encoding:"utf8",env:{...process.env,NEXTNC_PLAN:plan,NEXTNC_TOOL_TABLE:"",NEXTNC_DIAGNOSTICS:diagnostics}});
      const r = run([]); assert.equal(r.status,1); assert.equal(r.stdout,"");
      const record = JSON.parse(fs.readFileSync(path.join(diagnostics,"latest-error.json")));
      assert.equal(record.error.context.invariant,"POLICY_END_RETRACT_STATE");
      assert.equal(record.error.context.planPointer,"/end/0");
      assert.equal(record.inspection.validationCoverage.stages.completeness.status,"passed");
      assert.equal(record.inspection.validationCoverage.stages.policy.status,"failed");
      assert.equal(record.inspection.validationCoverage.stages.serialization.status,"not_checked");
      assert.match(renderReport(record), /POLICY_END_RETRACT_STATE/);
      if (command !== "filter") {
        const failed = run(["--output",output]); assert.equal(failed.status,1); assert.equal(failed.stdout,""); assert.ok(!fs.existsSync(output));
        fs.writeFileSync(output,"preserve"); assert.equal(run(["--output",output]).status,1); assert.equal(fs.readFileSync(output,"utf8"),"preserve"); fs.unlinkSync(output);
      }
    }
  } finally { fs.rmSync(dir,{recursive:true,force:true}); }
});
