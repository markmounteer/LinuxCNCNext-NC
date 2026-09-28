"use strict";
const {requireValue: need, correctionFor} = require("./errors");
const escape = value => String(value ?? "Not recorded").replace(/[&<>"']/g, c => ({"&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"}[c]));
const json = value => value === undefined ? "Not recorded" : JSON.stringify(value, null, 2);
const pre = value => `<pre>${escape(json(value))}</pre>`;
function table(rows) { return `<table><tbody>${rows.map(([label, value]) => `<tr><th>${escape(label)}</th><td>${escape(value)}</td></tr>`).join("")}</tbody></table>`; }
function grid(headers, rows) { return `<table><thead><tr>${headers.map(h => `<th>${escape(h)}</th>`).join("")}</tr></thead><tbody>${rows.map(row => `<tr>${row.map(cell => `<td>${escape(cell)}</td>`).join("")}</tr>`).join("")}</tbody></table>`; }
function renderReport(record) {
  need(record && typeof record === "object" && !Array.isArray(record) && record.schema === "linuxcnc-next-nc/diagnostic/1", "REPORT_SCHEMA", "Expected a linuxcnc-next-nc/diagnostic/1 archive; unsupported schemas are not rendered.");
  const inspection = record.inspection || {}, trace = inspection.traceability || {}, execution = inspection.execution || {};
  const errors = record.error?.context?.issues;
  const ranges = Array.isArray(trace.operationRanges) ? trace.operationRanges : [], sourceMap = Array.isArray(record.sourceMap) ? record.sourceMap : [];
  const groups = new Map();
  for (const op of ranges) {
    const key = JSON.stringify([op.tool, op.mappedTool, op.mappedWorkOffset]);
    if (!groups.has(key)) groups.set(key, {op, operations: []});
    groups.get(key).operations.push(op.section + ": " + op.operation);
  }
  const operationViews = ranges.map((op, index) => {
    const entries = sourceMap.filter(item => item.section === op.section);
    return `<article id="operation-${index}"><h3>${escape(op.section)}: ${escape(op.operation)}</h3>${table([["Fusion tool / offset", op.tool ? `${op.tool.number} / ${op.tool.offset}` : undefined], ["LinuxCNC T / H", op.mappedTool ? `${op.mappedTool.tool} / ${op.mappedTool.offset}` : undefined], ["LinuxCNC WCS", op.mappedWorkOffset], ["G-code line range", `${op.firstLine}–${op.lastLine}`]])}<details><summary>Show this operation's process state and source lines</summary>${pre({spindle: op.initialSpindle, coolant: op.initialCoolant})}${grid(["Line", "Phase", "Path / segment", "Recorded action"], entries.map(e => [e.line, e.phase, [e.path, e.segment].filter(x => x !== undefined).join(" / "), json(e.command ? {command: e.command, stateChange: e.stateChange, modalState: e.modalState, motion: e.motion} : e.motion || (e.action === "dwell" ? {seconds: e.seconds, position: e.position} : e.action))]))}</details></article>`;
  }).join("");
  const sections = [
    ["Identity and provenance", table([["Status", record.status], ["Recorded UTC", record.timeUTC], ["Translator", record.translator], ["Command", record.command], ["Machine", inspection.machine], ["Profile", inspection.profile], ["Units", inspection.units], ["Input", record.input], ["Input SHA-256", record.inputSHA256], ["Program fingerprint", inspection.programFingerprint?.value], ["Execution plan", record.plan], ["Plan SHA-256", record.planSHA256], ["Tool-table snapshot", record.toolTable], ["Tool-table SHA-256", record.toolTableSHA256], ["Output", record.output], ["Output SHA-256", record.outputSHA256], ["G-code SHA-256", trace.gcodeSHA256]])],
    ["Diagnostics", record.error ? `<p>${escape(correctionFor(record.error))}</p>` + pre(record.error) : "<p>No failure recorded in this archive.</p>"],
    ["Plan issues", Array.isArray(errors) ? errors.map(issue => `<article><h3>${escape(issue.code)} — ${escape(issue.context?.operation || issue.context?.field)}</h3><p>${escape(issue.message)}</p><p>${escape(issue.correction)}</p>${pre(issue.context)}</article>`).join("") : "<p>No aggregated issues recorded.</p>"],
    ["Checks not completed", pre(record.error?.context?.notChecked)],
    ["Operations, tools and process state", pre(inspection.operations)],
    ["Tools in use", groups.size ? grid(["Fusion tool:offset", "LinuxCNC T / H", "WCS", "Operations"], [...groups.values()].map(({op, operations}) => [op.tool ? `${op.tool.number}:${op.tool.offset}` : undefined, op.mappedTool ? `${op.mappedTool.tool} / ${op.mappedTool.offset}` : undefined, op.mappedWorkOffset, operations.join("; ")])) : "<p>Tool mappings not recorded.</p>"],
    ["Operation line ranges and mappings", ranges.length ? `<nav>${ranges.map((op, index) => `<p><a href="#operation-${index}">${escape(op.section)}: ${escape(op.operation)}</a> — lines ${escape(op.firstLine)}–${escape(op.lastLine)}</p>`).join("")}</nav>${operationViews}` : "<p>Line ranges not recorded.</p>"],
    ["Tool-table validation", pre(inspection.toolTable)],
    ["Transitions and execution", pre(execution)],
    ["Source map", `<p>Line numbers refer to the candidate G-code hash above. Preflight computes candidate lines but writes no G-code. Machine retracts have no invented starting position.</p><details><summary>Expand complete line-to-operation map</summary>${pre(record.sourceMap)}</details>`]
  ];
  return `<!doctype html>\n<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><title>Next-NC review report</title><style>body{font:16px system-ui,sans-serif;max-width:1100px;margin:2rem auto;padding:0 1rem;color:#172334;background:#f7f9fc}h1,h2{color:#123e66}section{background:white;padding:1rem;margin:1rem 0;border:1px solid #dce3eb;border-radius:8px}table{border-collapse:collapse;width:100%}th,td{text-align:left;vertical-align:top;border-bottom:1px solid #eee;padding:.45rem;overflow-wrap:anywhere}th{width:25%}pre{white-space:pre-wrap;overflow-wrap:anywhere;font:13px ui-monospace,monospace}article{border-left:3px solid #a34216;padding-left:1rem}summary{cursor:pointer}</style></head><body><h1>Next-NC review report</h1><p>Offline archive review. This report does not validate machine setup or authorize motion. Missing fields in older archives are shown as “Not recorded”.</p>${sections.map(([title, html]) => `<section><h2>${title}</h2>${html}</section>`).join("")}<details><summary>Complete source archive</summary>${pre(record)}</details></body></html>\n`;
}
module.exports = {renderReport};
