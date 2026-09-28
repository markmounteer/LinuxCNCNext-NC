"use strict";
// Contract for the two emitted Next-NC toolpath profiles, not an EXPRESS schema.
// Field order is Part 21 order, including inherited explicit attributes.
const string = {kind: "string"}, number = {kind: "number"}, integer = {kind: "integer"};
const ref = (...types) => ({kind: "reference", types});
const symbol = (...values) => ({kind: "symbol", values});
const optional = shape => ({kind: "choice", choices: [shape, symbol("$")]});
const list = (item, min, max = Infinity) => ({kind: "aggregate", item, min, max});
const typed = (...types) => ({kind: "typed", types});
const field = (attribute, shape) => ({attribute, shape});
const strings = (...names) => names.map(name => field(name, string));
const operations = ["TURNING_TYPE_OPERATION", "MILLING_TYPE_OPERATION"];
const methods = ["MACHINING_WORKPLAN", "MACHINING_WORKINGSTEP", ...operations, "MACHINING_FEATURE_PROCESS", "MACHINING_TECHNOLOGY", "MACHINING_FUNCTIONS", "MACHINING_TOOLPATH"];
const unit = ref("NAMED_UNIT", "CONTEXT_DEPENDENT_UNIT", "DERIVED_UNIT");
const rep = ref("REPRESENTATION", "MACHINING_SPINDLE_SPEED_REPRESENTATION", "MACHINING_FEED_SPEED_REPRESENTATION", "MACHINING_TOOLPATH_SPEED_PROFILE_REPRESENTATION");
const item = ref("DESCRIPTIVE_REPRESENTATION_ITEM", "MEASURE_REPRESENTATION_ITEM", "CARTESIAN_POINT", "POLYLINE", "TRIMMED_CURVE");
const context = ref("REPRESENTATION_CONTEXT");
const relation = [field("ACTION_METHOD_RELATIONSHIP.name", string), field("ACTION_METHOD_RELATIONSHIP.description", optional(string)), field("ACTION_METHOD_RELATIONSHIP.relating_method", ref(...methods)), field("ACTION_METHOD_RELATIONSHIP.related_method", ref(...methods))];
const representation = [field("REPRESENTATION.name", string), field("REPRESENTATION.items", list(item, 1)), field("REPRESENTATION.context_of_items", context)];
const product = [field("PRODUCT.id", string), field("PRODUCT.name", string), field("PRODUCT.description", optional(string)), field("PRODUCT.frame_of_reference", list(ref("PRODUCT_CONTEXT"), 1))];
const action = [field("ACTION.name", string), field("ACTION.description", optional(string)), field("ACTION.chosen_method", ref(...methods))];
const contract = {
  REPRESENTATION_CONTEXT: strings("REPRESENTATION_CONTEXT.context_identifier", "REPRESENTATION_CONTEXT.context_type"),
  LENGTH_UNIT: [], PLANE_ANGLE_UNIT: [], SOLID_ANGLE_UNIT: [], TIME_UNIT: [],
  NAMED_UNIT: [field("NAMED_UNIT.dimensions", {kind: "dimensions"})],
  SI_UNIT: [field("SI_UNIT.prefix", optional(symbol(".MILLI."))), field("SI_UNIT.name", symbol(".METRE.", ".RADIAN.", ".STERADIAN.", ".SECOND."))],
  GEOMETRIC_REPRESENTATION_CONTEXT: [field("GEOMETRIC_REPRESENTATION_CONTEXT.coordinate_space_dimension", integer)],
  GLOBAL_UNIT_ASSIGNED_CONTEXT: [field("GLOBAL_UNIT_ASSIGNED_CONTEXT.units", list(unit, 3, 3))],
  TIME_MEASURE_WITH_UNIT: [field("MEASURE_WITH_UNIT.value_component", typed("TIME_MEASURE")), field("MEASURE_WITH_UNIT.unit_component", unit)],
  LENGTH_MEASURE_WITH_UNIT: [field("MEASURE_WITH_UNIT.value_component", typed("LENGTH_MEASURE")), field("MEASURE_WITH_UNIT.unit_component", unit)],
  DIMENSIONAL_EXPONENTS: ["length", "mass", "time", "electric_current", "thermodynamic_temperature", "amount_of_substance", "luminous_intensity"].map(n => field(`DIMENSIONAL_EXPONENTS.${n}_exponent`, number)),
  CONVERSION_BASED_UNIT: [field("CONVERSION_BASED_UNIT.name", string), field("CONVERSION_BASED_UNIT.conversion_factor", ref("LENGTH_MEASURE_WITH_UNIT", "TIME_MEASURE_WITH_UNIT"))],
  CONTEXT_DEPENDENT_UNIT: [field("NAMED_UNIT.dimensions", ref("DIMENSIONAL_EXPONENTS")), field("CONTEXT_DEPENDENT_UNIT.name", string)],
  DERIVED_UNIT: [field("DERIVED_UNIT.elements", list(ref("DERIVED_UNIT_ELEMENT"), 2, 2))],
  DERIVED_UNIT_ELEMENT: [field("DERIVED_UNIT_ELEMENT.unit", ref("NAMED_UNIT", "CONTEXT_DEPENDENT_UNIT")), field("DERIVED_UNIT_ELEMENT.exponent", number)],
  APPLICATION_CONTEXT: strings("APPLICATION_CONTEXT.application"),
  APPLICATION_PROTOCOL_DEFINITION: [...strings("APPLICATION_PROTOCOL_DEFINITION.status", "APPLICATION_PROTOCOL_DEFINITION.application_interpreted_model_schema_name"), field("APPLICATION_PROTOCOL_DEFINITION.application_protocol_year", integer), field("APPLICATION_PROTOCOL_DEFINITION.application", ref("APPLICATION_CONTEXT"))],
  PRODUCT_CONTEXT: [field("APPLICATION_CONTEXT_ELEMENT.name", string), field("APPLICATION_CONTEXT_ELEMENT.frame_of_reference", ref("APPLICATION_CONTEXT")), field("PRODUCT_CONTEXT.discipline_type", string)],
  PRODUCT_DEFINITION_CONTEXT: [field("APPLICATION_CONTEXT_ELEMENT.name", string), field("APPLICATION_CONTEXT_ELEMENT.frame_of_reference", ref("APPLICATION_CONTEXT")), field("PRODUCT_DEFINITION_CONTEXT.life_cycle_stage", string)],
  PRODUCT: product, MACHINING_PROJECT: product,
  PRODUCT_DEFINITION_FORMATION: [field("PRODUCT_DEFINITION_FORMATION.id", string), field("PRODUCT_DEFINITION_FORMATION.description", optional(string)), field("PRODUCT_DEFINITION_FORMATION.of_product", ref("PRODUCT", "MACHINING_PROJECT"))],
  PRODUCT_DEFINITION: [field("PRODUCT_DEFINITION.id", string), field("PRODUCT_DEFINITION.description", optional(string)), field("PRODUCT_DEFINITION.formation", ref("PRODUCT_DEFINITION_FORMATION")), field("PRODUCT_DEFINITION.frame_of_reference", ref("PRODUCT_DEFINITION_CONTEXT"))],
  PRODUCT_DEFINITION_PROCESS: [...action, field("PRODUCT_DEFINITION_PROCESS.identification", string)],
  PROPERTY_PROCESS: [...action, field("PROPERTY_PROCESS.identification", string)],
  PROCESS_PRODUCT_ASSOCIATION: [...strings("PROCESS_PRODUCT_ASSOCIATION.name", "PROCESS_PRODUCT_ASSOCIATION.description"), field("PROCESS_PRODUCT_ASSOCIATION.defined_product", ref("PRODUCT_DEFINITION")), field("PROCESS_PRODUCT_ASSOCIATION.process", ref("PRODUCT_DEFINITION_PROCESS"))],
  PROCESS_PROPERTY_ASSOCIATION: [...strings("PROCESS_PROPERTY_ASSOCIATION.name", "PROCESS_PROPERTY_ASSOCIATION.description"), field("PROCESS_PROPERTY_ASSOCIATION.process", ref("PROPERTY_PROCESS")), field("PROCESS_PROPERTY_ASSOCIATION.property_or_shape", ref("INSTANCED_FEATURE"))],
  DESCRIPTIVE_REPRESENTATION_ITEM: strings("REPRESENTATION_ITEM.name", "DESCRIPTIVE_REPRESENTATION_ITEM.description"),
  ACTION_PROPERTY: [...strings("ACTION_PROPERTY.name", "ACTION_PROPERTY.description"), field("ACTION_PROPERTY.definition", ref(...methods))],
  ACTION_PROPERTY_REPRESENTATION: [...strings("ACTION_PROPERTY_REPRESENTATION.name", "ACTION_PROPERTY_REPRESENTATION.description"), field("ACTION_PROPERTY_REPRESENTATION.property", ref("ACTION_PROPERTY")), field("ACTION_PROPERTY_REPRESENTATION.representation", rep)],
  MACHINING_PROJECT_WORKPIECE_RELATIONSHIP: [...strings("PRODUCT_DEFINITION_RELATIONSHIP.id", "PRODUCT_DEFINITION_RELATIONSHIP.name"), field("PRODUCT_DEFINITION_RELATIONSHIP.description", optional(string)), field("PRODUCT_DEFINITION_RELATIONSHIP.relating_product_definition", ref("PRODUCT_DEFINITION")), field("PRODUCT_DEFINITION_RELATIONSHIP.related_product_definition", ref("PRODUCT_DEFINITION"))],
  PRODUCT_DEFINITION_SHAPE: [field("PROPERTY_DEFINITION.name", string), field("PROPERTY_DEFINITION.description", optional(string)), field("PROPERTY_DEFINITION.definition", ref("PRODUCT_DEFINITION"))],
  INSTANCED_FEATURE: [field("CHARACTERIZED_OBJECT.name", string), field("CHARACTERIZED_OBJECT.description", optional(string)), field("SHAPE_ASPECT.name", string), field("SHAPE_ASPECT.description", optional(string)), field("SHAPE_ASPECT.of_shape", ref("PRODUCT_DEFINITION_SHAPE")), field("SHAPE_ASPECT.product_definitional", symbol(".T.", ".F.", ".U."))],
  ACTION_RESOURCE_TYPE: strings("ACTION_RESOURCE_TYPE.name"),
  MACHINING_TOOL: [field("ACTION_RESOURCE.name", string), field("ACTION_RESOURCE.description", optional(string)), field("ACTION_RESOURCE.usage", list(ref(...operations), 1)), field("ACTION_RESOURCE.kind", ref("ACTION_RESOURCE_TYPE"))],
  CARTESIAN_POINT: [field("REPRESENTATION_ITEM.name", string), field("CARTESIAN_POINT.coordinates", list(number, 3, 3))],
  DIRECTION: [field("REPRESENTATION_ITEM.name", string), field("DIRECTION.direction_ratios", list(number, 3, 3))],
  MEASURE_REPRESENTATION_ITEM: [field("REPRESENTATION_ITEM.name", string), field("MEASURE_WITH_UNIT.value_component", typed("NUMERIC_MEASURE", "TIME_MEASURE")), field("MEASURE_WITH_UNIT.unit_component", unit)],
  POLYLINE: [field("REPRESENTATION_ITEM.name", string), field("POLYLINE.points", list(ref("CARTESIAN_POINT"), 2))],
  AXIS2_PLACEMENT_3D: [field("REPRESENTATION_ITEM.name", string), field("PLACEMENT.location", ref("CARTESIAN_POINT")), field("AXIS2_PLACEMENT_3D.axis", ref("DIRECTION")), field("AXIS2_PLACEMENT_3D.ref_direction", ref("DIRECTION"))],
  CIRCLE: [field("REPRESENTATION_ITEM.name", string), field("CONIC.position", ref("AXIS2_PLACEMENT_3D")), field("CIRCLE.radius", number)],
  TRIMMED_CURVE: [field("REPRESENTATION_ITEM.name", string), field("TRIMMED_CURVE.basis_curve", ref("CIRCLE")), ...["trim_1", "trim_2"].map(n => field(`TRIMMED_CURVE.${n}`, list({kind: "choice", choices: [ref("CARTESIAN_POINT"), typed("PARAMETER_VALUE")]}, 1, 1))), field("TRIMMED_CURVE.sense_agreement", symbol(".T.", ".F.")), field("TRIMMED_CURVE.master_representation", symbol(".CARTESIAN.", ".PARAMETER."))]
};
for (const type of methods) contract[type] = [field("ACTION_METHOD.name", string), field("ACTION_METHOD.description", optional(string)), ...strings("ACTION_METHOD.consequence", "ACTION_METHOD.purpose")];
for (const type of ["REPRESENTATION", "MACHINING_SPINDLE_SPEED_REPRESENTATION", "MACHINING_FEED_SPEED_REPRESENTATION", "MACHINING_TOOLPATH_SPEED_PROFILE_REPRESENTATION"]) contract[type] = representation;
for (const type of ["MACHINING_OPERATION_RELATIONSHIP", "MACHINING_TECHNOLOGY_RELATIONSHIP", "MACHINING_FUNCTIONS_RELATIONSHIP", "MACHINING_FEATURE_RELATIONSHIP"]) contract[type] = relation;
for (const type of ["MACHINING_PROCESS_SEQUENCE_RELATIONSHIP", "MACHINING_TOOLPATH_SEQUENCE_RELATIONSHIP"]) contract[type] = [...relation, field("SEQUENTIAL_METHOD.sequence_position", number)];
const complexCombinations = [
  ["LENGTH_UNIT", "NAMED_UNIT", "SI_UNIT"], ["NAMED_UNIT", "PLANE_ANGLE_UNIT", "SI_UNIT"],
  ["NAMED_UNIT", "SI_UNIT", "SOLID_ANGLE_UNIT"], ["NAMED_UNIT", "SI_UNIT", "TIME_UNIT"],
  ["CONVERSION_BASED_UNIT", "LENGTH_UNIT", "NAMED_UNIT"], ["CONVERSION_BASED_UNIT", "NAMED_UNIT", "TIME_UNIT"],
  ["GEOMETRIC_REPRESENTATION_CONTEXT", "GLOBAL_UNIT_ASSIGNED_CONTEXT", "REPRESENTATION_CONTEXT"]
];
module.exports = {contract, complexCombinations, version: "next-nc/profile-shape/1"};
