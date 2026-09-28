"use strict";
// Independent reader for the Part 21 subset emitted by Next-NC, not an interpreter.
function decodeString(value) {
  return value.replace(/''/g, "'").replace(/\\\\|\\X2\\([0-9A-Fa-f]+)\\X0\\/g, (match, hex) => {
    if (!hex) return "\\";
    if (hex.length % 4) throw new Error("Invalid X2 string escape");
    return hex.match(/.{4}/g).map(part => String.fromCharCode(parseInt(part, 16))).join("");
  });
}
function parse(text) {
  text = text.replace(/\r\n/g, "\n"); // Native Windows post engine uses CRLF.
  const start = text.indexOf("\nDATA;\n");
  if (!text.startsWith("ISO-10303-21;\nHEADER;\n") || start < 0 || !text.endsWith("ENDSEC;\nEND-ISO-10303-21;\n")) throw new Error("Incomplete Part 21 document");
  const data = text.slice(start + 7, text.lastIndexOf("ENDSEC;"));
  const tokens = data.match(/'(?:[^']|'')*'|#[0-9]+|[A-Z_][A-Z_0-9]*|\.[A-Z_]+\.|[-+]?(?:\d+\.\d*|\d*\.\d+|\d+)(?:E[-+]?\d+)?|[(),;=$*]|\S/g) || [];
  let cursor = 0;
  function take(t) { const value = tokens[cursor++]; if (t && value !== t) throw new Error(`Expected ${t}, found ${value}`); return value; }
  function values() {
    take("("); const result = [];
    if (tokens[cursor] !== ")") { do { result.push(value()); } while (tokens[cursor] === "," && take(",")); }
    take(")"); return result;
  }
  function entity() { const type = take(); if (!/^[A-Z_][A-Z_0-9]*$/.test(type)) throw new Error("Invalid entity type " + type); return {type, args: values()}; }
  function value() {
    const token = tokens[cursor];
    if (token === "(") return values();
    if (/^#/.test(token)) {
      take(); const ref = Number(token.slice(1));
      if (!Number.isSafeInteger(ref) || ref < 1) throw new Error("Invalid reference");
      return {ref};
    }
    if (/^'/.test(token)) { take(); return decodeString(token.slice(1, -1)); }
    if (/^[A-Z_]/.test(token)) return entity();
    take();
    if (["$", "*"].includes(token) || /^\.[A-Z_]+\.$/.test(token)) return {symbol: token};
    const number = Number(token); if (!Number.isFinite(number)) throw new Error(`Invalid value ${token}`); return number;
  }
  const records = new Map();
  while (cursor < tokens.length) {
    const id = take(); if (!/^#[1-9][0-9]*$/.test(id)) throw new Error("Invalid record ID"); take("=");
    const parts = [];
    if (tokens[cursor] === "(") { take("("); while (tokens[cursor] !== ")") parts.push(entity()); take(")"); }
    else parts.push(entity());
    take(";");
    const n = Number(id.slice(1));
    if (!Number.isSafeInteger(n)) throw new Error("Invalid record ID");
    if (records.has(n)) throw new Error("Duplicate record ID"); records.set(n, parts);
  }
  function references(value) {
    if (Array.isArray(value)) value.forEach(references);
    else if (value && typeof value === "object") {
      if (value.ref && !records.has(value.ref)) throw new Error(`Missing reference #${value.ref}`);
      if (value.args) references(value.args);
    }
  }
  for (const parts of records.values()) references(parts);
  return {
    records,
    all(type) { return [...records.values()].flat().filter(e => e.type === type); },
    get(ref) { return records.get(ref.ref); }
  };
}
module.exports = {parse};
