"use strict";
const crypto = require("node:crypto");
const {requireValue: need} = require("./errors");
const limitations = [
  "Checks a file snapshot, not the table currently loaded by LinuxCNC or an external tool database.",
  "Does not verify physical tool identity, geometry, measured offsets, changer pockets or machine configuration.",
  "Does not verify work offsets, travel, clearance, spindle feedback or safe execution."
];
function checkToolTable(text, program, plan) {
  if (text === undefined) return {status: "not_checked", reason: "No LinuxCNC tool-table snapshot supplied.", limitations};
  need(typeof text === "string" && Buffer.byteLength(text, "utf8") <= 1024 * 1024, "TOOL_TABLE", "Tool table must be UTF-8 text of at most 1 MiB.");
  const records = new Map();
  for (const [index, raw] of text.replace(/^\uFEFF/, "").split(/\r?\n/).entries()) {
    const line = raw.split(";")[0].trim();
    if (!line) continue;
    const values = {}, context = {toolTableLine: index + 1};
    for (const word of line.split(/\s+/)) {
      const match = /^([TPXYZABCUVWDIJQ])([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?)$/i.exec(word);
      need(match, "TOOL_TABLE_SYNTAX", `Unrecognized tool-table word on line ${index + 1}: ${word}`, context);
      const key = match[1].toUpperCase(), value = Number(match[2]);
      need(!Object.hasOwn(values, key) && Number.isFinite(value), "TOOL_TABLE_SYNTAX", `Duplicate or nonfinite ${key} on tool-table line ${index + 1}.`, context);
      if (key === "T" || key === "P") need(/^\+?\d+$/.test(match[2]) && Number.isSafeInteger(value), "TOOL_TABLE_SYNTAX", `${key} must be a nonnegative integer written without a decimal point or exponent.`, context);
      if (key === "Q") need(/^\+?\d+$/.test(match[2]) && value <= 9, "TOOL_TABLE_SYNTAX", "Lathe orientation Q must be an integer from 0 through 9.", context);
      values[key] = value;
    }
    need(Object.hasOwn(values, "T") && Object.hasOwn(values, "P"), "TOOL_TABLE_SYNTAX", `Tool-table line ${index + 1} requires T and P.`, context);
    need(!records.has(values.T), "TOOL_TABLE_DUPLICATE", `Tool T${values.T} appears more than once in the supplied table.`, context);
    records.set(values.T, {line: index + 1});
  }
  const mappings = [], missing = [];
  program.model.sections.forEach((section, index) => {
    const fusionTool = `${section.tool.number}:${section.tool.offset}`, mapping = plan.tools[fusionTool];
    const item = {section: index + 1, operation: section.name, fusionTool, tool: mapping.tool, offset: mapping.offset};
    const absent = [];
    if (!records.has(mapping.tool)) absent.push({role: "selected tool", record: mapping.tool});
    if (!records.has(mapping.offset)) absent.push({role: "H-offset", record: mapping.offset});
    if (absent.length) missing.push({...item, missing: absent});
    else mappings.push({...item, toolTableLine: records.get(mapping.tool).line, offsetTableLine: records.get(mapping.offset).line});
  });
  need(!missing.length, "TOOL_TABLE_MISSING", `${missing.length} operation(s) reference tool or H-offset records absent from the supplied LinuxCNC table. Check the plan mappings and table; no G-code was written.`, {operations: missing});
  return {status: "passed", scope: "File syntax and presence of mapped T/H records only", sha256: crypto.createHash("sha256").update(text).digest("hex"), records: records.size, mappings, limitations};
}
module.exports = {checkToolTable};
