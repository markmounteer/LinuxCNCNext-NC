"use strict";
const {ValidationError} = require("../vendor/fusion360next-nc/validation-error");
const {parse} = require("../vendor/fusion360next-nc/part21");
const {inspectDocument} = require("../vendor/fusion360next-nc/inspect");
const {NextNCError, requireValue: need} = require("./errors");
const {exitPoint, continuation, connection} = require("./continuity");
// Intentionally closed to additional executable semantics. This is the emitted
// Next-NC profile, not a general AP238 or arbitrary STEP interpreter.
const {validateProfileShape} = require("./profile-shape");
const {coverageTracker} = require("./validation-coverage");
const propertyNames = {
  MACHINING_WORKPLAN: ["next-nc profile", "next-nc coordinates"],
  TURNING_TYPE_OPERATION: ["next-nc tool offset", "next-nc work offset", "next-nc entry point"],
  MACHINING_TECHNOLOGY: ["spindle", "feedrate", "feedrate reference"],
  MACHINING_FUNCTIONS: ["coolant", "coolant type"],
  MACHINING_TOOLPATH: ["priority", "trajectory type", "direction", "basic curve", "speed profile", "dwell"]
};
propertyNames.MILLING_TYPE_OPERATION = propertyNames.TURNING_TYPE_OPERATION;
const operations = ["TURNING_TYPE_OPERATION", "MILLING_TYPE_OPERATION"];
const endpoints = {
  MACHINING_PROCESS_SEQUENCE_RELATIONSHIP: ["MACHINING_WORKPLAN", "MACHINING_WORKINGSTEP"],
  MACHINING_OPERATION_RELATIONSHIP: ["MACHINING_WORKINGSTEP", operations],
  MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP: [operations, "MACHINING_TOOLPATH"],
  MACHINING_TECHNOLOGY_RELATIONSHIP: [[...operations, "MACHINING_TOOLPATH"], "MACHINING_TECHNOLOGY"],
  MACHINING_FUNCTIONS_RELATIONSHIP: [[...operations, "MACHINING_TOOLPATH"], "MACHINING_FUNCTIONS"],
  MACHINING_FEATURE_RELATIONSHIP: ["MACHINING_WORKINGSTEP", "MACHINING_FEATURE_PROCESS"]
};
function readProgram(text) {
  const validation = coverageTracker(); validation.start("part21");
  try {
    need(typeof text === "string" && Buffer.byteLength(text) <= 32 * 1024 * 1024, "INPUT_SIZE", "Input must be text no larger than 32 MiB.");
    const doc = parse(text), properties = new Map(), propertyRecords = new Map(), propertyRepresentations = new Map();
    validation.pass({records: doc.records.size}); validation.start("profileShape");
    need(doc.schema === "INTEGRATED_CNC_SCHEMA", "SCHEMA", "Expected the Next-NC INTEGRATED_CNC_SCHEMA header.");
    const shape = validateProfileShape(doc);
    validation.pass(shape); validation.start("semantics");
    const single = ref => { const parts = doc.get(ref); need(parts?.length === 1, "PROFILE", "Expected a single entity reference."); return parts[0]; };
    for (const e of doc.all("ACTION_PROPERTY_REPRESENTATION")) {
      need(single(e.args[2]).type === "ACTION_PROPERTY" && e.args[0] === "" && e.args[1] === "", "PROFILE", "Unexpected property representation association.");
      propertyRepresentations.set(e.args[2].ref, single(e.args[3]));
    }
    for (const [id, parts] of doc.records) for (const e of parts) {
      if (e.type === "ACTION_PROPERTY") {
        const owner = single(e.args[2]);
        need(propertyNames[owner.type]?.includes(e.args[0]), "UNSUPPORTED_PROPERTY", `Unsupported ${owner.type} property '${e.args[0]}' at #${id}.`);
        const names = properties.get(e.args[2].ref) || []; names.push(e.args[0]); properties.set(e.args[2].ref, names);
        const rep = propertyRepresentations.get(id);
        need(rep && e.args[1] === "", "PROFILE", `Unexpected property semantics at #${id}.`);
        propertyRecords.set(e.args[2].ref + "|" + e.args[0], rep);
        if (!["spindle", "feedrate", "basic curve", "speed profile", "dwell", "next-nc entry point"].includes(e.args[0])) {
          need(rep.type === "REPRESENTATION" && rep.args[1].length === 1, "PROFILE", "Invalid descriptive property representation.");
          const item = single(rep.args[1][0]);
          need(item.type === "DESCRIPTIVE_REPRESENTATION_ITEM" && item.args[0] === e.args[0], "PROFILE", `Descriptive property label disagrees at #${id}.`);
        }
      }
      if (endpoints[e.type]) {
        for (let i = 0; i < 2; ++i) need([endpoints[e.type][i]].flat().includes(single(e.args[i + 2]).type), "PROFILE", `Unexpected ${e.type} endpoint at #${id}.`);
        need(e.args[0] === "" && e.args[1] === "", "UNSUPPORTED_RELATIONSHIP", `Unexpected relationship semantics at #${id}.`);
      }
      if (["MACHINING_WORKPLAN", "MACHINING_WORKINGSTEP", ...operations, "MACHINING_TOOLPATH", "MACHINING_TECHNOLOGY", "MACHINING_FUNCTIONS", "MACHINING_FEATURE_PROCESS"].includes(e.type)) {
        need(e.args[2] === "" && e.args[3] === "", "UNSUPPORTED_METHOD", `Unexpected action purpose/consequence at #${id}.`);
        if (e.type === "MACHINING_WORKINGSTEP") need(e.args[1] === "machining", "UNSUPPORTED_METHOD", "Unsupported workingstep classification.");
        if (["MACHINING_WORKPLAN", ...operations].includes(e.type)) need(e.args[1] === "", "UNSUPPORTED_METHOD", "Unsupported operation classification.");
      }
    }
    for (const [id, parts] of doc.records) {
      const e = parts[0], names = properties.get(id) || [];
      if (e.type === "MACHINING_TOOLPATH") {
        const dwell = e.args[1] === "feedstop";
        need(dwell ? names.length === 2 && names.includes("priority") && names.includes("dwell") : !names.includes("dwell"), "AMBIGUOUS_PATH", `Unexpected motion/dwell properties at #${id}.`);
      }
      if (e.type === "MACHINING_FUNCTIONS") {
        const rep = propertyRecords.get(id + "|coolant");
        if (rep?.args[1]?.length === 1 && single(rep.args[1][0]).args[1] === "coolant off") need(!names.includes("coolant type"), "AMBIGUOUS_COOLANT", "Coolant off cannot also specify a coolant type.");
      }
      if (e.type === "MACHINING_TECHNOLOGY_RELATIONSHIP" && operations.includes(single(e.args[2]).type)) {
        need(!(properties.get(e.args[3].ref) || []).includes("feedrate"), "AMBIGUOUS_FEED", "Operation initial state cannot carry an unused cutting feed.");
      }
    }
    const result = inspectDocument(doc);
    for (const [index, section] of result.model.sections.entries()) {
      const context = {section: index + 1, operation: section.name, tool: section.tool.number};
      result.report.operations[index].entry = section.start;
      result.report.operations[index].exit = exitPoint(section);
      result.report.operations[index].continuation = continuation(result, index);
      result.report.operations[index].connection = connection(result, index);
      need(section.initialCoolant !== "through tool", "COOLANT", "Through-tool coolant needs a separately specified machine mapping and is unsupported.", context);
      for (const [p, path] of section.paths.entries()) {
        need(path.coolant !== "through tool", "COOLANT", "Through-tool coolant is unsupported.", {...context, path: p + 1});
        need(path.kind !== "dwell" || path.feed === null, "AMBIGUOUS_PATH", "A dwell cannot carry a cutting feed.", {...context, path: p + 1});
      }
    }
    validation.pass(); result.report.validationCoverage = validation.coverage;
    return result;
  } catch (error) {
    const failure = error instanceof ValidationError ? new NextNCError("INVALID_NEXTNC", error.message, error.context) : error;
    validation.fail(failure); throw failure;
  }
}
module.exports = {readProgram};
