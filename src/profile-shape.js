"use strict";
const {contract, complexCombinations, version} = require("./profile-contract");
const {NextNCError} = require("./errors");
const signatures = new Set(complexCombinations.map(parts => [...parts].sort().join("+")));
const complexOnly = new Set(complexCombinations.flat().filter(type => type !== "REPRESENTATION_CONTEXT"));
function actualKind(value) {
  if (value === undefined) return "missing";
  if (Array.isArray(value)) return `aggregate[${value.length}]`;
  if (typeof value !== "object" || value === null) return typeof value;
  if (value.ref !== undefined) return `reference #${value.ref}`;
  if (value.symbol !== undefined) return value.symbol;
  if (value.type) return `typed ${value.type}[${value.args.length}]`;
  return "object";
}
function describe(shape) {
  switch (shape.kind) {
    case "reference": return "reference to " + shape.types.join(" | ");
    case "symbol": return shape.values.join(" | ");
    case "choice": return shape.choices.map(describe).join(" | ");
    case "aggregate": return `aggregate[${shape.min}:${shape.max === Infinity ? "?" : shape.max}] of ${describe(shape.item)}`;
    case "typed": return shape.types.join(" | ") + "(finite number)";
    case "number": return "finite number";
    case "integer": return "safe integer";
    default: return shape.kind;
  }
}
function matches(value, shape, doc) {
  switch (shape.kind) {
    case "string": return typeof value === "string";
    case "number": return typeof value === "number" && Number.isFinite(value);
    case "integer": return Number.isSafeInteger(value);
    case "symbol": return shape.values.includes(value?.symbol);
    case "reference": return Number.isSafeInteger(value?.ref) && doc.records.get(value.ref)?.some(part => shape.types.includes(part.type));
    case "choice": return shape.choices.some(s => matches(value, s, doc));
    case "aggregate": return Array.isArray(value) && value.length >= shape.min && value.length <= shape.max && value.every(v => matches(v, shape.item, doc));
    case "typed": return shape.types.includes(value?.type) && value.args.length === 1 && matches(value.args[0], {kind: "number"}, doc);
    default: throw new Error("Unknown internal profile shape " + shape.kind);
  }
}
function validateProfileShape(doc) {
  let components = 0, attributes = 0;
  for (const [id, parts] of doc.records) {
    const types = parts.map(p => p.type), signature = [...types].sort().join("+");
    const base = {stage: "profile-shape", record: "#" + id, ...doc.locations.get(id)};
    const fail = (component, attribute, parameterIndex, expected, actual) => {
      const context = {...base, component, attribute, parameterIndex, expected, actual};
      throw new NextNCError("PROFILE_SHAPE", `#${id} ${component}, attribute ${attribute} (parameter ${parameterIndex ?? "n/a"}): expected ${expected}; found ${actual}.`, context);
    };
    for (const type of types) if (!Object.hasOwn(contract, type)) throw new NextNCError("UNSUPPORTED_ENTITY", `Unsupported entity: #${id} ${type}.`, {...base, component: type});
    if (parts.length > 1 ? !signatures.has(signature) : complexOnly.has(types[0])) fail(signature, "<components>", null, "a supported unit/context component combination", signature);
    for (const part of parts) {
      ++components;
      const fields = contract[part.type];
      if (part.args.length !== fields.length) {
        const index = Math.min(part.args.length, fields.length);
        fail(part.type, fields[index]?.attribute || "<extra parameter>", index + 1, `${fields.length} parameters`, `${part.args.length} parameters`);
      }
      for (const [index, field] of fields.entries()) {
        let shape = field.shape;
        if (shape.kind === "dimensions") shape = types.includes("SI_UNIT") ? {kind: "symbol", values: ["*"]} : {kind: "reference", types: ["DIMENSIONAL_EXPONENTS"]};
        if (!matches(part.args[index], shape, doc)) fail(part.type, field.attribute, index + 1, describe(shape), actualKind(part.args[index]));
        ++attributes;
      }
    }
  }
  return {contract: version, records: doc.records.size, components, attributes};
}
module.exports = {validateProfileShape};
