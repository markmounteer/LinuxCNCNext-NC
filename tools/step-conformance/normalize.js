"use strict";
// Independent adapter. Never import the production string decoder or profile code.
function decode(raw) {
  if (raw[0] !== "'" || raw.at(-1) !== "'") throw new Error("Invalid quoted string");
  let result = "";
  for (let i = 1; i < raw.length - 1;) {
    const char = raw[i++];
    if (char === "'") { if (raw[i++] !== "'") throw new Error("Undoubled apostrophe"); result += "'"; }
    else if (char !== "\\") result += char;
    else if (raw[i] === "\\") { result += "\\"; ++i; }
    else {
      if (raw.slice(i, i + 3) !== "X2\\") throw new Error("Unsupported string escape");
      i += 3; const end = raw.indexOf("\\X0\\", i), hex = raw.slice(i, end);
      if (end < 0 || !/^(?:[0-9a-fA-F]{4})+$/.test(hex)) throw new Error("Malformed X2 escape");
      const units = [];
      for (let h = 0; h < hex.length; h += 4) units.push(Number.parseInt(hex.slice(h, h + 4), 16));
      result += String.fromCharCode(...units); i = end + 4;
    }
  }
  return result;
}
const numeric = n => {
  if (!Number.isFinite(n)) throw new Error("Nonfinite number");
  return ["number", Object.is(n, -0) ? "-0" : String(n)];
};
const id = n => { if (!Number.isSafeInteger(n) || n <= 0) throw new Error("Unsafe or nonpositive ID"); return n; };
function upstreamValue(node, depth = 0) {
  if (depth > 64) throw new Error("Nesting limit");
  const next = v => upstreamValue(v, depth + 1);
  switch (node.type) {
    case "IntegerValue": case "RealValue": return numeric(node.value);
    case "StringValue": return ["string", decode(node.value)];
    case "EnumerationValue": return ["symbol", node.value];
    case "EntityRef": return ["reference", id(node.id)];
    case "NullParameter": return ["symbol", "$"];
    case "OmittedParameter": return ["symbol", "*"];
    case "List": return ["aggregate", node.items.map(next)];
    case "TypedParameter": return ["typed", node.keyword, [next(node.parameter)]];
    default: throw new Error("Unsupported oracle AST value " + node.type);
  }
}
function targetValue(value) {
  if (Array.isArray(value)) return ["aggregate", value.map(targetValue)];
  if (typeof value === "number") return numeric(value);
  if (typeof value === "string") return ["string", value];
  if (value.ref !== undefined) return ["reference", id(value.ref)];
  if (value.symbol !== undefined) return ["symbol", value.symbol];
  return ["typed", value.type, value.args.map(targetValue)];
}
// Records by ID, components by name; all parameter and aggregate order, including
// duplicate uses, is retained. Integer/real spellings share exact JS numeric value.
const components = parts => parts.sort((a,b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
function normalizeUpstream(ast) {
  const seen = new Set();
  return {header: ast.header.entities.map(e => [e.keyword, e.parameters.map(p => upstreamValue(p))]),
    records: ast.data.flatMap(d => d.entities).map(e => {
      id(e.id); if (seen.has(e.id)) throw new Error("Duplicate record ID"); seen.add(e.id);
      const parts = e.type === "SimpleEntityInstance" ? [e.record] : e.records;
      if (new Set(parts.map(p => p.keyword)).size !== parts.length || !parts.length) throw new Error("Duplicate/empty components");
      return [e.id, components(parts.map(p => [p.keyword, p.parameters.map(v => upstreamValue(v))]))];
    }).sort((a,b) => a[0] - b[0])};
}
function normalizeTarget(doc) {
  return {header: doc.header.map(e => [e.type, e.args.map(targetValue)]),
    records: [...doc.records].map(([n, parts]) => [n, components(parts.map(p => [p.type, p.args.map(targetValue)]))]).sort((a,b) => a[0] - b[0])};
}
function references(tree) {
  const ids = new Set(tree.records.map(r => r[0]));
  function visit(value) {
    if (!Array.isArray(value)) return;
    if (value[0] === "reference" && !ids.has(value[1])) throw new Error(`Missing reference #${value[1]}`);
    value.forEach(visit);
  }
  tree.records.forEach(visit);
}
function envelope(text, ast) {
  if (ast.data.length !== 1 || ast.data[0].name !== undefined || ast.data[0].parameters !== undefined || ast.anchor || ast.reference || ast.signatures.length) throw new Error("Unsupported document sections");
  if (text.slice(ast.span.end.offset).trim() !== "") throw new Error("Unconsumed trailing content");
  if (ast.header.entities.map(e => e.keyword).join(",") !== "FILE_DESCRIPTION,FILE_NAME,FILE_SCHEMA") throw new Error("Unexpected header entities");
}
module.exports = {normalizeUpstream, normalizeTarget, references, envelope, decode};
