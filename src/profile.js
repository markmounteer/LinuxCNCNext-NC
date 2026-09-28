"use strict";
const {parse} = require("../vendor/fusion360next-nc/part21");
const {inspect} = require("../vendor/fusion360next-nc/inspect");
const {NextNCError, requireValue: need} = require("./errors");
// Intentionally closed to additional executable semantics. This is the emitted
// Next-NC profile, not a general AP238 or arbitrary STEP interpreter.
const arities = {
  REPRESENTATION_CONTEXT: 2, LENGTH_UNIT: 0, NAMED_UNIT: 1, SI_UNIT: 2, PLANE_ANGLE_UNIT: 0,
  SOLID_ANGLE_UNIT: 0, GEOMETRIC_REPRESENTATION_CONTEXT: 1, GLOBAL_UNIT_ASSIGNED_CONTEXT: 1,
  TIME_UNIT: 0, TIME_MEASURE_WITH_UNIT: 2, LENGTH_MEASURE_WITH_UNIT: 2, DIMENSIONAL_EXPONENTS: 7,
  CONVERSION_BASED_UNIT: 2, CONTEXT_DEPENDENT_UNIT: 2, DERIVED_UNIT: 1, DERIVED_UNIT_ELEMENT: 2,
  APPLICATION_CONTEXT: 1, APPLICATION_PROTOCOL_DEFINITION: 4, PRODUCT_CONTEXT: 3,
  PRODUCT_DEFINITION_CONTEXT: 3, MACHINING_PROJECT: 4, PRODUCT_DEFINITION_FORMATION: 3,
  PRODUCT_DEFINITION: 4, MACHINING_WORKPLAN: 4, PRODUCT_DEFINITION_PROCESS: 4,
  PROCESS_PRODUCT_ASSOCIATION: 4, DESCRIPTIVE_REPRESENTATION_ITEM: 2, REPRESENTATION: 3,
  ACTION_PROPERTY: 3, ACTION_PROPERTY_REPRESENTATION: 4, PRODUCT: 4,
  MACHINING_PROJECT_WORKPIECE_RELATIONSHIP: 5, PRODUCT_DEFINITION_SHAPE: 3,
  MACHINING_WORKINGSTEP: 4, MACHINING_PROCESS_SEQUENCE_RELATIONSHIP: 5, TURNING_TYPE_OPERATION: 4,
  MACHINING_OPERATION_RELATIONSHIP: 4, INSTANCED_FEATURE: 6, MACHINING_FEATURE_PROCESS: 4,
  PROPERTY_PROCESS: 4, PROCESS_PROPERTY_ASSOCIATION: 4, MACHINING_FEATURE_RELATIONSHIP: 4,
  ACTION_RESOURCE_TYPE: 1, MACHINING_TOOL: 4, CARTESIAN_POINT: 2, DIRECTION: 2,
  MACHINING_TECHNOLOGY: 4, MEASURE_REPRESENTATION_ITEM: 3, MACHINING_SPINDLE_SPEED_REPRESENTATION: 3,
  MACHINING_FEED_SPEED_REPRESENTATION: 3, MACHINING_TOOLPATH_SPEED_PROFILE_REPRESENTATION: 3,
  MACHINING_TECHNOLOGY_RELATIONSHIP: 4, MACHINING_FUNCTIONS: 4, MACHINING_FUNCTIONS_RELATIONSHIP: 4,
  MACHINING_TOOLPATH: 4, MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP: 5, POLYLINE: 2,
  AXIS2_PLACEMENT_3D: 4, CIRCLE: 3, TRIMMED_CURVE: 6
};
const propertyNames = {
  MACHINING_WORKPLAN: ["next-nc profile", "next-nc coordinates"],
  TURNING_TYPE_OPERATION: ["next-nc tool offset", "next-nc work offset", "next-nc entry point"],
  MACHINING_TECHNOLOGY: ["spindle", "feedrate", "feedrate reference"],
  MACHINING_FUNCTIONS: ["coolant", "coolant type"],
  MACHINING_TOOLPATH: ["priority", "trajectory type", "direction", "basic curve", "speed profile", "dwell"]
};
const endpoints = {
  MACHINING_PROCESS_SEQUENCE_RELATIONSHIP: ["MACHINING_WORKPLAN", "MACHINING_WORKINGSTEP"],
  MACHINING_OPERATION_RELATIONSHIP: ["MACHINING_WORKINGSTEP", "TURNING_TYPE_OPERATION"],
  MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP: ["TURNING_TYPE_OPERATION", "MACHINING_TOOLPATH"],
  MACHINING_TECHNOLOGY_RELATIONSHIP: [["TURNING_TYPE_OPERATION", "MACHINING_TOOLPATH"], "MACHINING_TECHNOLOGY"],
  MACHINING_FUNCTIONS_RELATIONSHIP: [["TURNING_TYPE_OPERATION", "MACHINING_TOOLPATH"], "MACHINING_FUNCTIONS"],
  MACHINING_FEATURE_RELATIONSHIP: ["MACHINING_WORKINGSTEP", "MACHINING_FEATURE_PROCESS"]
};
function readProgram(text) {
  try {
    need(typeof text === "string" && Buffer.byteLength(text) <= 32 * 1024 * 1024, "INPUT_SIZE", "Input must be text no larger than 32 MiB.");
    const header = text.replace(/\r\n/g, "\n").split("\nDATA;\n")[0];
    need(/^FILE_SCHEMA\(\('INTEGRATED_CNC_SCHEMA'\)\);$/m.test(header), "SCHEMA", "Expected the Next-NC INTEGRATED_CNC_SCHEMA header.");
    const doc = parse(text), properties = new Map(), propertyRecords = new Map(), propertyRepresentations = new Map();
    const single = ref => { const parts = doc.get(ref); need(parts?.length === 1, "PROFILE", "Expected a single entity reference."); return parts[0]; };
    for (const e of doc.all("ACTION_PROPERTY_REPRESENTATION")) {
      need(single(e.args[2]).type === "ACTION_PROPERTY" && e.args[0] === "" && e.args[1] === "", "PROFILE", "Unexpected property representation association.");
      propertyRepresentations.set(e.args[2].ref, single(e.args[3]));
    }
    for (const [id, parts] of doc.records) for (const e of parts) {
      need(Object.hasOwn(arities, e.type) && e.args.length === arities[e.type], "UNSUPPORTED_ENTITY", `Unsupported entity or attribute count: #${id} ${e.type}.`);
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
      if (["MACHINING_WORKPLAN", "MACHINING_WORKINGSTEP", "TURNING_TYPE_OPERATION", "MACHINING_TOOLPATH", "MACHINING_TECHNOLOGY", "MACHINING_FUNCTIONS", "MACHINING_FEATURE_PROCESS"].includes(e.type)) {
        need(e.args[2] === "" && e.args[3] === "", "UNSUPPORTED_METHOD", `Unexpected action purpose/consequence at #${id}.`);
        if (e.type === "MACHINING_WORKINGSTEP") need(e.args[1] === "machining", "UNSUPPORTED_METHOD", "Unsupported workingstep classification.");
        if (["MACHINING_WORKPLAN", "TURNING_TYPE_OPERATION"].includes(e.type)) need(e.args[1] === "", "UNSUPPORTED_METHOD", "Unsupported operation classification.");
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
      if (e.type === "MACHINING_TECHNOLOGY_RELATIONSHIP" && single(e.args[2]).type === "TURNING_TYPE_OPERATION") {
        need(!(properties.get(e.args[3].ref) || []).includes("feedrate"), "AMBIGUOUS_FEED", "Operation initial state cannot carry an unused cutting feed.");
      }
    }
    const result = inspect(text);
    for (const [index, section] of result.model.sections.entries()) {
      const context = {section: index + 1, operation: section.name, tool: section.tool.number};
      need(section.initialCoolant !== "through tool", "COOLANT", "Through-tool coolant needs a separately specified machine mapping and is unsupported in 0.1.0.", context);
      for (const [p, path] of section.paths.entries()) {
        need(path.coolant !== "through tool", "COOLANT", "Through-tool coolant is unsupported in 0.1.0.", {...context, path: p + 1});
        need(path.kind !== "dwell" || path.feed === null, "AMBIGUOUS_PATH", "A dwell cannot carry a cutting feed.", {...context, path: p + 1});
      }
    }
    return result;
  } catch (error) {
    if (error instanceof NextNCError) throw error;
    throw new NextNCError("INVALID_NEXTNC", error.message);
  }
}
module.exports = {readProgram};
