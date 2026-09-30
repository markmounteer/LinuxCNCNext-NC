"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const {translate} = require("../src/translate"), {renderReport} = require("../src/report");
const {semanticFixture, referenceGcode} = require("./support/semantic-fixture");
const {example} = require("../scripts/example");
const get = (phase, kind) => phase.quantities.find(q => q.kind === kind);
for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) test(`${machine} ${units}: process quantities include later changes, inherited state and source/line bindings`, () => {
  const f = semanticFixture(machine, units), r = translate(f.text, f.plan), p = r.report.processSummary;
  assert.equal(p.schema, "linuxcnc-next-nc/process-summary/1"); assert.equal(p.gcodeSHA256, r.report.traceability.gcodeSHA256);
  assert.equal(p.coordinates, machine === "mill" ? "XYZ Cartesian" : "XZ, X radius");
  assert.equal(r.gcode.split("\n").filter(l => !l.startsWith("(")).join("\n"), referenceGcode(machine, units));
  const initial = p.operations[0].phases[0], speed = get(initial, "spindleSpeed");
  assert.deepEqual([speed.source.value, speed.source.unit, speed.output.value, speed.output.unit], machine === "mill" ? [600, "RPM", 600, "RPM"] : units === "mm" ? [80000, "mm/min", 80, "m/min"] : [1200, "in/min", 100, "ft/min"]);
  assert.equal(get(initial, "spindleDirection").output.value, "clockwise");
  if (machine === "lathe") assert.equal(get(initial, "maximumRPM").output.value, 1800);
  assert.equal(get(p.operations[1].phases[0], "spindleSpeed").disposition, "inherited");
  const later = p.operations[2].phases.at(-1), feed = get(later, "feed");
  assert.equal(get(later, "spindleSpeed").output.value, 700); assert.equal(get(later, "spindleSpeed").source.unit, "RPM");
  assert.equal(get(later, "spindleDirection").output.value, "counterclockwise"); assert.equal(get(later, "coolant").output.value, "mist");
  assert.deepEqual([feed.output.value, feed.output.unit, feed.mode], [0.2, units === "mm" ? "mm/rev" : "in/rev", "perRevolution"]);
  const dwell = p.operations[2].phases.find(q => q.kind === "dwell");
  assert.equal(get(dwell, "dwell").output.value, 0.25); assert.equal(get(dwell, "dwell").output.unit, "s");
  assert.equal(dwell.commanded.feedRate, 100); assert.equal(dwell.commandedUnits.feedRate, units === "mm" ? "mm/min" : "in/min");
  for (const op of p.operations) for (const phase of op.phases) {
    assert.ok(phase.source.operation.record); assert.ok(phase.source.workingstep.sourceLine > 0);
    if (phase.path) assert.ok(phase.source.toolpath.record);
    for (const q of phase.quantities) {
      assert.ok(q.source.provenance);
      for (const line of q.lines) { const entry = r.sourceMap[line - 1]; assert.equal(entry.section, op.section); assert.equal(entry.path || null, phase.path || null); assert.equal(entry.provenance.origin, "step"); }
    }
  }
  const processLines = new Set(p.operations.flatMap(op => op.phases.flatMap(phase => phase.quantities.flatMap(q => q.lines))));
  for (const e of r.sourceMap.filter(e => e.provenance.origin === "step" && ["spindleMode","spindleStop","spindleStart","coolant","feed","dwell"].includes(e.command.type))) assert.ok(processLines.has(e.line), `Unreported process command at line ${e.line}`);
  const change = p.policyCommands.find(e => e.type === "toolChange"); assert.ok(change.invalidated.includes("coolant"));
  const modeOnly = p.policyCommands.find(e => e.instruction === "G94"); assert.deepEqual(modeOnly.quantities, []);
  const html = renderReport({schema: "linuxcnc-next-nc/diagnostic/1", inspection: r.report, sourceMap: r.sourceMap});
  assert.match(html, /700 RPM/); assert.match(html, /0.25 s/); assert.match(html, /counterclockwise/); assert.match(html, /inherited \(no new command\)/);
});
test("summary restores M6 coolant and preserves old archive uncertainty and escaped text", () => {
  const f = example(), r = translate(f.text, f.plan), initial = r.report.processSummary.operations[0].phases[0];
  assert.equal(get(initial, "coolant").source.value, "off"); assert.equal(get(initial, "coolant").output.value, "off"); assert.equal(get(initial, "coolant").disposition, "inherited");
  const archive = {schema: "linuxcnc-next-nc/diagnostic/1", inspection: structuredClone(r.report), sourceMap: r.sourceMap};
  const oldCoolant = get(archive.inspection.processSummary.operations[0].phases[0], "coolant");
  oldCoolant.output.value = null; oldCoolant.disposition = "unknown";
  assert.match(renderReport(archive), /Unknown/);
  const hostile = '</td><script>alert("x")</script>';
  archive.inspection.processSummary.operations[0].phases[0].quantities[0].source.value = hostile;
  const before = JSON.stringify(archive), html = renderReport(archive);
  assert.equal((html.match(/<script>/g) || []).length, 1); assert.doesNotMatch(html, /<script>alert/); assert.match(html, /&lt;script&gt;/); assert.equal(JSON.stringify(archive), before);
  delete archive.inspection.processSummary;
  assert.match(renderReport(archive), /Process summary: Not recorded/);
});
for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) test(`${machine} ${units}: job requirements use all paths and distinguish snapshot checks from commissioning`, () => {
  const f = semanticFixture(machine, units);
  for (const table of [undefined, f.toolTable]) {
    const r = translate(f.text, f.plan, {toolTable: table}), q = r.report.jobRequirements;
    assert.equal(q.machine, machine); assert.equal(q.units, units); assert.equal(q.spindle.number, 0);
    assert.deepEqual(q.axes, machine === "mill" ? ["X", "Y", "Z"] : ["X", "Z"]);
    assert.deepEqual(q.arcPlanes, machine === "mill" ? ["YZ", "XZ", "XY"] : ["XZ"]);
    assert.deepEqual(q.feedModes, ["perMinute", "perRevolution"]); assert.deepEqual(q.spindle.directions, ["clockwise", "counterclockwise"]);
    assert.deepEqual(q.coolant.requested, ["flood", "mist"]); assert.deepEqual(q.coolant.commands, ["off", "flood", "mist"]);
    assert.deepEqual(q.boundaries, {continue: 1, link: 1, retract: 2});
    assert.equal(q.evidence.translation.status, "passed"); assert.deepEqual(q.evidence.toolTable, r.report.toolTable);
    assert.equal(q.evidence.toolTable.status, table ? "passed" : "not_checked"); assert.equal(q.evidence.controller.status, "not_checked");
    assert.ok(q.evidence.controller.checks.every(c => c.status === "not_checked"));
    assert.ok(q.evidence.controller.checks.some(c => c.id === "spindle-feedback"));
    assert.equal(q.evidence.controller.checks.some(c => c.id === "css-origin"), machine === "lathe");
    const rpm = q.quantityRanges.find(v => v.kind === "spindleSpeed" && v.mode === "rpm");
    assert.deepEqual(rpm.source, {unit: "RPM", min: machine === "lathe" ? 700 : 600, max: 900});
    const minute = q.quantityRanges.find(v => v.kind === "feed" && v.mode === "perMinute");
    assert.deepEqual(minute.source, {unit: units === "mm" ? "mm/min" : "in/min", min: 60, max: 100});
    const rev = q.quantityRanges.find(v => v.kind === "feed" && v.mode === "perRevolution");
    assert.deepEqual(rev.source, {unit: units === "mm" ? "mm/rev" : "in/rev", min: 0.2, max: 0.2}); assert.deepEqual(rev.sections, [3]);
    assert.deepEqual(q.mappings.map(v => [v.tool, v.offset, v.workOffset]), [[1,3,"G54"],[1,3,"G54"],[1,3,"G54"],[2,1,"G55"]]);
    const html = renderReport({schema: "linuxcnc-next-nc/diagnostic/1", inspection: r.report, sourceMap: r.sourceMap});
    assert.match(html, /spindle\.0\.speed-in/); assert.match(html, /Controller commissioning/); assert.match(html, /Requested quantity ranges/);
    assert.ok(html.includes(`${minute.source.min}–${minute.source.max} ${minute.source.unit}`));
  }
});
