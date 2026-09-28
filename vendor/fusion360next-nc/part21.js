"use strict";
const {ValidationError} = require("./validation-error");
// Independent reader for the Part 21 subset emitted by Next-NC, not an interpreter.
function decodeString(value) {
  return value.replace(/''/g, "'").replace(/\\\\|\\X2\\([0-9A-Fa-f]+)\\X0\\/g, (match, hex) => {
    if (!hex) return "\\";
    if (hex.length % 4) throw new ValidationError("Invalid X2 string escape", {stage: "parse", rule: "STRING_ESCAPE"});
    return hex.match(/.{4}/g).map(part => String.fromCharCode(parseInt(part, 16))).join("");
  });
}
function parse(text) {
  if (typeof text !== "string" || Buffer.byteLength(text) > 32 * 1024 * 1024) throw new ValidationError("Input must be text no larger than 32 MiB", {stage: "parse", rule: "INPUT_SIZE"});
  text = text.replace(/\r\n/g, "\n"); // Native Windows post engine uses CRLF.
  const lineStarts = [0];
  for (let i = 0; i < text.length; ++i) if (text[i] === "\n") lineStarts.push(i + 1);
  function location(offset) {
    let lo = 0, hi = lineStarts.length;
    while (lo + 1 < hi) { const mid = (lo + hi) >>> 1; if (lineStarts[mid] <= offset) lo = mid; else hi = mid; }
    return {sourceLine: lo + 1, sourceColumn: offset - lineStarts[lo] + 1};
  }
  const start = text.indexOf("\nDATA;\n");
  if (!text.startsWith("ISO-10303-21;\nHEADER;\n") || start < 0 || !text.endsWith("ENDSEC;\nEND-ISO-10303-21;\n")) throw new ValidationError("Incomplete Part 21 document", {stage: "parse", rule: "ENVELOPE"});
  let tokens, offsets, cursor = 0, depth = 0, activeRecord;
  function tokenize(from, to) {
    const matches = [...text.slice(from, to).matchAll(/'(?:[^']|'')*'|#[0-9]+|[A-Z_][A-Z_0-9]*|\.[A-Z_]+\.|[-+]?(?:\d+\.\d*|\d*\.\d+|\d+)(?:E[-+]?\d+)?|[(),;=$*]|\S/g)];
    tokens = matches.map(m => m[0]); offsets = matches.map(m => from + m.index); offsets.push(to); cursor = 0;
  }
  function fail(message, offset = offsets[Math.min(cursor, offsets.length - 1)]) {
    const error = new ValidationError(message, {stage: "parse", rule: "SYNTAX", ...location(offset)});
    if (activeRecord) error.context.record = "#" + activeRecord;
    throw error;
  }
  function take(t) { const value = tokens[cursor]; if (value === undefined || (t && value !== t)) fail(`Expected ${t || "token"}, found ${value === undefined ? "end of section" : value}`); ++cursor; return value; }
  function values() {
    if (++depth > 64) fail("Part 21 nesting exceeds 64 levels");
    take("("); const result = [];
    if (tokens[cursor] !== ")") { do { result.push(value()); } while (tokens[cursor] === "," && take(",")); }
    take(")"); --depth; return result;
  }
  function entity() { const type = take(); if (!/^[A-Z_][A-Z_0-9]*$/.test(type)) fail("Invalid entity type " + type); return {type, args: values()}; }
  function value() {
    const token = tokens[cursor];
    if (token === "(") return values();
    if (/^#/.test(token)) {
      take(); const ref = Number(token.slice(1));
      if (!Number.isSafeInteger(ref) || ref < 1) fail("Invalid reference");
      return {ref};
    }
    if (/^'/.test(token)) {
      if (token.length < 2 || !token.endsWith("'")) fail("Unterminated Part 21 string");
      take(); try { return decodeString(token.slice(1, -1)); } catch (error) { if (!(error instanceof ValidationError)) throw error; fail(error.message); }
    }
    if (/^[A-Z_]/.test(token)) return entity();
    take();
    if (["$", "*"].includes(token) || /^\.[A-Z_]+\.$/.test(token)) return {symbol: token};
    const number = Number(token); if (!Number.isFinite(number)) fail(`Invalid value ${token}`); return number;
  }
  tokenize("ISO-10303-21;\nHEADER;\n".length, start);
  const header = [];
  for (const type of ["FILE_DESCRIPTION", "FILE_NAME", "FILE_SCHEMA"]) {
    const entry = entity(); if (entry.type !== type) fail("Expected " + type + " header"); take(";"); header.push(entry);
  }
  take("ENDSEC"); take(";");
  if (cursor !== tokens.length) fail("Unexpected header content");
  const strings = v => Array.isArray(v) && v.length > 0 && v.every(s => typeof s === "string");
  const [description, name, schema] = header.map(h => h.args);
  if (description.length !== 2 || !strings(description[0]) || typeof description[1] !== "string" ||
    name.length !== 7 || ![0, 1, 4, 5, 6].every(i => typeof name[i] === "string") || !strings(name[2]) || !strings(name[3]) ||
    schema.length !== 1 || !strings(schema[0]) || schema[0].length !== 1) fail("Invalid Next-NC Part 21 header attributes");
  tokenize(start + 7, text.lastIndexOf("ENDSEC;"));
  const records = new Map(), locations = new Map(), types = new Map();
  while (cursor < tokens.length) {
    const offset = offsets[cursor], id = take(); if (!/^#[1-9][0-9]*$/.test(id)) fail("Invalid record ID", offset);
    const n = Number(id.slice(1)); activeRecord = n;
    if (!Number.isSafeInteger(n)) fail("Invalid record ID", offset);
    take("=");
    const parts = [];
    if (tokens[cursor] === "(") { take("("); while (tokens[cursor] !== ")") parts.push(entity()); take(")"); }
    else parts.push(entity());
    take(";");
    if (!parts.length || new Set(parts.map(p => p.type)).size !== parts.length) fail("Empty or duplicate complex entity components", offset);
    if (records.has(n)) fail("Duplicate record ID", offset); records.set(n, parts); locations.set(n, location(offset));
    for (const part of parts) { if (!types.has(part.type)) types.set(part.type, []); types.get(part.type).push(part); }
  }
  function references(value) {
    if (Array.isArray(value)) value.forEach(references);
    else if (value && typeof value === "object") {
      if (value.ref && !records.has(value.ref)) {
        throw new ValidationError(`Missing reference #${value.ref}`, {stage: "parse", rule: "MISSING_REFERENCE", ...locations.get(activeRecord), record: "#" + activeRecord, missingRecord: "#" + value.ref});
      }
      if (value.args) references(value.args);
    }
  }
  for (const [id, parts] of records) { activeRecord = id; references(parts); }
  return {
    records, locations, header, schema: schema[0][0],
    all(type) { return types.get(type) || []; },
    get(ref) { return records.get(ref.ref); }
  };
}
module.exports = {parse};
