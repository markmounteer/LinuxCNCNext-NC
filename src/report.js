"use strict";
const {requireValue: need, correctionFor} = require("./errors");
const {reviewGeometry} = require("./review-geometry"), {stockPreview} = require("./stock-preview"), {viewer} = require("./review-viewer");
const escape = value => String(value ?? "Not recorded").replace(/[&<>"']/g, c => ({"&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"}[c]));
const json = value => value === undefined ? "Not recorded" : JSON.stringify(value, null, 2);
const pre = value => `<pre>${escape(json(value))}</pre>`;
function table(rows) { return `<table><tbody>${rows.map(([label, value]) => `<tr><th>${escape(label)}</th><td>${escape(value)}</td></tr>`).join("")}</tbody></table>`; }
function grid(headers, rows) { return `<table><thead><tr>${headers.map(h => `<th>${escape(h)}</th>`).join("")}</tr></thead><tbody>${rows.map(row => `<tr>${row.map(cell => `<td>${escape(cell)}</td>`).join("")}</tr>`).join("")}</tbody></table>`; }
const quantity = q => q?.value == null ? "Unknown" : `${q.value}${q.unit ? " " + q.unit : ""}`;
const label = value => ({spindleSpeed: "Spindle speed", spindleDirection: "Spindle direction", maximumRPM: "Maximum RPM", coolant: "Coolant", feed: "Feed", dwell: "Dwell", css: "CSS (G96)", rpm: "RPM (G97)", perMinute: "Per minute (G94)", perRevolution: "Per revolution (G95)", not_checked: "Not checked"}[value] || value);
function sourceRecords(value) {
  const records = new Set();
  function visit(v) { if (!v || typeof v !== "object") return; if (v.record) records.add(v.record); for (const item of Object.values(v)) visit(item); }
  visit(value); return [...records].join(", ");
}
function processView(op) {
  if (!op) return "<p>Process summary: Not recorded.</p>";
  return op.phases.map(p => `<h4>${escape(p.phase === "initial" ? "Initial settings" : `Path ${p.path}: ${p.kind}`)}</h4><p>Context lines ${escape(p.firstLine)}–${escape(p.lastLine)}; source ${escape(sourceRecords(p.source))}.</p>` +
    grid(["Quantity", "Decoded source", "LinuxCNC commanded", "Emission / lines", "Conversion / source records"], p.quantities.map(q => [label(q.kind) + (q.mode ? " / " + label(q.mode) : ""), quantity(q.source), quantity(q.output), q.disposition + (q.lines.length ? ": " + q.lines.join(", ") : " (no new command)"), q.conversion + "; " + sourceRecords(q.source.provenance)])) +
    `<details><summary>Retained commanded state (unknown values preserved)</summary>${table(Object.entries(p.commanded).map(([key, value]) => [key, quantity({value, unit: p.commandedUnits[key]})]))}</details>`).join("");
}
function requirementsView(r) {
  if (r?.schema !== "linuxcnc-next-nc/job-requirements/1") return "<p>Job requirements: Not recorded.</p>";
  const range = v => v.min == null ? "Unknown" : `${v.min}–${v.max} ${v.unit}`;
  return table([["Machine / axes", `${r.machine} / ${r.axes.join(", ")}`], ["Coordinates / units", `${r.coordinates} / ${r.units}`], ["Arc planes", r.arcPlanes.join(", ") || "None"],
    ["Spindle", `${r.spindle.number}: ${r.spindle.modes.map(label).join(", ")}; ${r.spindle.directions.join(", ")}`], ["Feed modes", r.feedModes.map(label).join(", ")], ["Requested coolant", r.coolant.requested.join(", ")],
    ["Coolant commands", r.coolant.commands.join(", ")], ["Reviewed boundaries", `continue ${r.boundaries.continue}; link ${r.boundaries.link}; retract ${r.boundaries.retract}`],
    ["Translation checks", r.evidence.translation.status + ": " + r.evidence.translation.scope], ["Table snapshot", label(r.evidence.toolTable.status)], ["Controller commissioning", label(r.evidence.controller.status)]]) +
    `<h3>Requested quantity ranges</h3>` + grid(["Quantity", "Decoded source range", "Commanded range", "Sections"], r.quantityRanges.map(q => [label(q.kind) + (q.mode ? " / " + label(q.mode) : ""), range(q.source), range(q.output), q.sections.join(", ")])) +
    `<h3>Controller requirements — not checked</h3>` + grid(["Requirement", "Status", "Meaning"], r.evidence.controller.checks.map(c => [c.id, label(c.status), c.detail])) +
    r.limitations.map(l => `<p>${escape(l)}</p>`).join("");
}
function identityView(identity) {
  const status = identity?.schema === "linuxcnc-next-nc/artifact-identity/1" ? identity.status : "not_checked";
  const labels = {match: "Match", mismatch: "Mismatch", not_recorded: "Not recorded", invalid_hash: "Invalid recorded hash", not_checked: "Not checked"};
  const explanation = status === "match" ? "The selected file bytes match the archived candidate hash. This does not establish authenticity, execution or what the controller currently has loaded." :
    "All operation line ranges and source mappings below describe the archived candidate. They are not verified for a selected file.";
  return `<p><strong>Saved G-code identity: ${escape(labels[status] || "Not checked")}</strong></p><p>${explanation}</p>` +
    (status === "not_checked" ? "<p>No saved G-code file was checked in this review.</p>" : table([["Selected file", identity.file], ["Bytes read", identity.bytes], ["Checked UTC", identity.checkedUTC], ["Expected candidate SHA-256", identity.expectedSHA256], ["Observed file SHA-256", identity.observedSHA256], ["Scope", identity.scope]]));
}
function validationView(coverage) {
  if (coverage?.schema !== "linuxcnc-next-nc/validation-coverage/1") return "<p>Not recorded.</p>";
  return "<p>Profile validation covers the supported toolpath subset. General EXPRESS and full AP238 conformance are separate checks.</p>" +
    grid(["Stage", "Status", "Scope", "Evidence"], Object.entries(coverage.stages).map(([name, stage]) => {
      const {status, scope, ...evidence} = stage;
      return [name, label(status), scope, Object.keys(evidence).length ? json(evidence) : "None recorded"];
    }));
}
function motionView(review) {
  if (!review) return "<p>Motion metrics: Not recorded or source map unavailable.</p>";
  const n = v => Number(v.toPrecision(9)), t = review.totals;
  return table([["Known cutting distance", `${n(t.cuttingDistance)} ${review.units}`], ["Known rapid distance", `${n(t.rapidDistance)} ${review.units}`],
    ["Motion blocks / distance unknown", `${t.motionBlocks} / ${t.unknownDistanceBlocks}`], ["Programmed dwell", `${n(t.dwellSeconds)} s`],
    ["Ideal feed time for resolved cutting blocks", `${n(t.idealFeedSeconds)} s`], ["Cutting blocks without a time estimate", t.unknownFeedTimeBlocks],
    ["Rapid / tool-change / spindle-delay time", "Not estimated; no total cycle-time claim"]]) +
    grid(["Operation", `Cutting (${review.units})`, `Rapid (${review.units})`, "Distance unknown (blocks)", "Ideal feed (s)", "Feed time unknown (blocks)", "Dwell (s)"],
      review.operations.map(op => [`${op.section}: ${op.operation}`, n(op.cuttingDistance), n(op.rapidDistance), op.unknownDistanceBlocks, n(op.idealFeedSeconds), op.unknownFeedTimeBlocks, n(op.dwellSeconds)])) +
    review.limitations.map(l => `<p>${escape(l)}</p>`).join("");
}
function stockView(stock) {
  if (!stock) return "<p>Not requested. Optional XYZ mill preview requires an explicit stock-and-tool setup file with --stock-setup. See docs/visual-review.md.</p>";
  const n = v => Number(v.toPrecision(8)), line = v => `<button data-review-line="${v}">Line ${v}</button>`;
  return `<p><strong>Approximate final stock surface — not a collision or clearance certificate.</strong> Pale blue is original height; orange/brown is deeper removal. This shows the final state after resolved cutting moves, independently of the selected toolpath line.</p><canvas id="stock-canvas" width="800" height="440" aria-label="Approximate final stock top surface"></canvas>` +
    table([["Stock frame / units", `${stock.workOffset} / ${stock.units}`], ["Grid", `${stock.nx} × ${stock.ny}; cells ${n(stock.dx)} × ${n(stock.dy)} ${stock.units}`],
      ["Initial / remaining volume", `${n(stock.initialVolume)} / ${n(stock.remainingVolume)} ${stock.units}³`], ["Approximate removed volume", `${n(stock.removedVolume)} ${stock.units}³`],
      ["Sampled path points", stock.samples], ["Setup SHA-256 (parsed JSON)", stock.setupSHA256]]) +
    `<h3>Sampled rapid intersections</h3><p>${stock.rapidIntersections.length ? stock.rapidIntersections.map(line).join(" ") : "None found by this sampled model. Excluded moves and unmodeled objects are not checked."}</p>` +
    `<h3>Excluded moves</h3>${stock.unresolvedLines.length ? stock.unresolvedLines.map(v => `<p>${line(v.line)} ${escape(v.reason)}</p>`).join("") : "<p>None.</p>"}` +
    stock.limitations.map(l => `<p>${escape(l)}</p>`).join("");
}
function renderReport(record, {artifactIdentity, stockSetup} = {}) {
  need(record && typeof record === "object" && !Array.isArray(record) && record.schema === "linuxcnc-next-nc/diagnostic/1", "REPORT_SCHEMA", "Expected a linuxcnc-next-nc/diagnostic/1 archive; unsupported schemas are not rendered.");
  const inspection = record.inspection || {}, trace = inspection.traceability || {}, execution = inspection.execution || {};
  const errors = record.error?.context?.issues;
  const ranges = Array.isArray(trace.operationRanges) ? trace.operationRanges : [], sourceMap = Array.isArray(record.sourceMap) ? record.sourceMap : [];
  let review, unavailable = "No complete typed source map is available in this archive.";
  if (sourceMap.length) {
    try { review = reviewGeometry(inspection, sourceMap); }
    catch (error) { if (error.code !== "REVIEW_GEOMETRY") throw error; unavailable = error.message; }
  }
  if (stockSetup) need(review, "STOCK_PREVIEW", "Stock preview requires a complete usable source map: " + unavailable);
  const stock = stockSetup ? stockPreview(review, stockSetup) : null;
  const visual = review ? viewer(review, stock) : {html: `<p>Toolpath viewer unavailable: ${escape(unavailable)}</p>`, csp: "", script: ""};
  const groups = new Map();
  for (const op of ranges) {
    const key = JSON.stringify([op.tool, op.mappedTool, op.mappedWorkOffset]);
    if (!groups.has(key)) groups.set(key, {op, operations: []});
    groups.get(key).operations.push(op.section + ": " + op.operation);
  }
  const operationViews = ranges.map((op, index) => {
    const entries = sourceMap.filter(item => item.section === op.section);
    const process = inspection.processSummary?.schema === "linuxcnc-next-nc/process-summary/1" ? inspection.processSummary.operations.find(p => p.section === op.section) : null;
    return `<article id="operation-${index}"><h3>${escape(op.section)}: ${escape(op.operation)}</h3>${table([["Fusion tool / offset", op.tool ? `${op.tool.number} / ${op.tool.offset}` : undefined], ["LinuxCNC T / H", op.mappedTool ? `${op.mappedTool.tool} / ${op.mappedTool.offset}` : undefined], ["LinuxCNC WCS", op.mappedWorkOffset], ["G-code line range", `${op.firstLine}–${op.lastLine}`]])}${processView(process)}<details><summary>Show this operation's detailed source lines</summary>${pre({initialSpindle: op.initialSpindle, initialCoolant: op.initialCoolant})}${grid(["Line", "Phase", "Path / segment", "Recorded action"], entries.map(e => [e.line, e.phase, [e.path, e.segment].filter(x => x !== undefined).join(" / "), json(e.command ? {command: e.command, stateChange: e.stateChange, modalState: e.modalState, motion: e.motion, provenance: e.provenance} : e.motion || (e.action === "dwell" ? {seconds: e.seconds, position: e.position} : e.action))]))}</details></article>`;
  }).join("");
  const sections = [
    ["Saved G-code identity", identityView(artifactIdentity)],
    ["Toolpath review", visual.html],
    ["Motion metrics", motionView(review || inspection.motionSummary)],
    ["Optional stock preview", stockView(stock)],
    ["Identity and provenance", table([["Status", record.status], ["Recorded UTC", record.timeUTC], ["Translator", record.translator], ["Command", record.command], ["Machine", inspection.machine], ["Profile", inspection.profile], ["Units", inspection.units], ["Input", record.input], ["Input SHA-256", record.inputSHA256], ["Program fingerprint", inspection.programFingerprint?.value], ["Execution plan", record.plan], ["Plan SHA-256", record.planSHA256], ["Tool-table snapshot", record.toolTable], ["Tool-table SHA-256", record.toolTableSHA256], ["Output", record.output], ["Output SHA-256", record.outputSHA256], ["G-code SHA-256", trace.gcodeSHA256], ["Source records bound to input SHA-256", trace.provenance?.inputSHA256]])],
    ["Diagnostics", record.error ? `<p>${escape(correctionFor(record.error))}</p>` + pre(record.error) : "<p>No failure recorded in this archive.</p>"],
    ["Validation coverage", validationView(inspection.validationCoverage)],
    ["Translator policy audit", execution.policy?.schema === "linuxcnc-next-nc/policy-audit/1" ? pre(execution.policy) : "<p>Not recorded.</p>"],
    ["Final G-code serialization audit", execution.serialization?.schema === "linuxcnc-next-nc/serialization-audit/1" ? pre(execution.serialization) : "<p>Not recorded.</p>"],
    ["Plan issues", Array.isArray(errors) ? errors.map(issue => `<article><h3>${escape(issue.code)} — ${escape(issue.context?.operation || issue.context?.field)}</h3><p>${escape(issue.message)}</p><p>${escape(issue.correction)}</p>${pre(issue.context)}</article>`).join("") : "<p>No aggregated issues recorded.</p>"],
    ["Checks not completed", pre(record.error?.context?.notChecked)],
    ["Job requirements", requirementsView(inspection.jobRequirements)],
    ["Source operation details", `<details><summary>Show original source inspection</summary>${pre(inspection.operations)}</details>`],
    ["Tools in use", groups.size ? grid(["Fusion tool:offset", "LinuxCNC T / H", "WCS", "Operations"], [...groups.values()].map(({op, operations}) => [op.tool ? `${op.tool.number}:${op.tool.offset}` : undefined, op.mappedTool ? `${op.mappedTool.tool} / ${op.mappedTool.offset}` : undefined, op.mappedWorkOffset, operations.join("; ")])) : "<p>Tool mappings not recorded.</p>"],
    ["Operation line ranges and mappings", ranges.length ? `<nav>${ranges.map((op, index) => `<p><a href="#operation-${index}">${escape(op.section)}: ${escape(op.operation)}</a> — lines ${escape(op.firstLine)}–${escape(op.lastLine)}</p>`).join("")}</nav>${operationViews}` : "<p>Line ranges not recorded.</p>"],
    ["Tool-table validation", pre(inspection.toolTable)],
    ["Translator policy commands", inspection.processSummary?.schema === "linuxcnc-next-nc/process-summary/1" ? `<p>Initialization, transition and shutdown commands; these are not requested cutting-speed ranges. Unknown state after M6 is not machine feedback.</p>` + grid(["Line", "Context", "Instruction", "Quantities", "State made unknown"], inspection.processSummary.policyCommands.map(e => [e.line, `${e.phase}${e.section ? "; section " + e.section : ""}`, e.instruction, e.quantities.map(quantity).join("; "), e.invalidated.join(", ")])) : "<p>Policy command summary: Not recorded.</p>"],
    ["Transitions and execution", pre(execution)],
    ["Source map", `<p>Line numbers refer to the candidate G-code hash above. Preflight computes candidate lines but writes no G-code. Machine retracts have no invented starting position.</p><details><summary>Expand complete line-to-operation map</summary>${pre(record.sourceMap)}</details>`]
  ];
  return `<!doctype html>\n<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; ${visual.csp} style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><title>Next-NC review report</title><style>body{font:16px system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;color:#172334;background:#f7f9fc}h1,h2{color:#123e66}section{background:white;padding:1rem;margin:1rem 0;border:1px solid #dce3eb;border-radius:8px}table{border-collapse:collapse;width:100%}th,td{text-align:left;vertical-align:top;border-bottom:1px solid #eee;padding:.45rem;overflow-wrap:anywhere}tbody th{width:25%}pre{white-space:pre-wrap;overflow-wrap:anywhere;font:13px ui-monospace,monospace}article{border-left:3px solid #a34216;padding-left:1rem}summary{cursor:pointer}canvas{width:100%;height:auto;border:1px solid #dce3eb}button,input,select{font:inherit;padding:.35rem;max-width:100%}button{cursor:pointer}.review-controls{display:flex;flex-wrap:wrap;align-items:center;gap:.7rem;margin:.8rem 0}.review-controls label{display:flex;gap:.4rem;align-items:center}#review-line{width:6rem}#review-slider,#line-list{width:100%}#line-command{background:#eef2f8;padding:.7rem}#line-detail{max-height:24rem;overflow:auto}</style></head><body><h1>Next-NC review report</h1><p>Offline archive review. This report does not validate machine setup or authorize motion. Missing fields in older archives are shown as “Not recorded”.</p>${sections.map(([title, html]) => `<section><h2>${title}</h2>${html}</section>`).join("")}<details><summary>Complete source archive</summary>${pre(record)}</details>${visual.script}</body></html>\n`;
}
module.exports = {renderReport};
