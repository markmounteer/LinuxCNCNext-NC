"use strict";
const {semanticFixture} = require("../../test/support/semantic-fixture");
const {geometryFixture} = require("../../test/support/geometry-fixture");
const {Program} = require("../../vendor/fusion360next-nc/next-nc");
// Named controls are tied to source.json's parser commit. Their differing stages
// are explicit expectations, never a blanket diagnostic suppression.
const pass = {syntax: "passed", envelope: "passed", normalization: "passed", references: "passed", targetParse: "passed", targetProfile: "passed", agreement: "passed"};
function cases() {
  const result = [];
  for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
    const text = semanticFixture(machine, units).text.replace(/\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z/g, "2026-09-28T00:00:00.000Z");
    const add = (name, input, expected = pass, discrepancy) => result.push({name: `${machine}-${units}-${name}`, text: input, expected, discrepancy});
    add("process", text);
    // Record renumbering/reversal and CRLF/whitespace test parsing independently.
    const lines = text.replace(/#(\d+)/g, (_, n) => "#" + (Number(n) + 700)).split("\n"), records = lines.filter(l => l.startsWith("#")).reverse();
    add("formatted", lines.map(l => l.startsWith("#") ? "  " + records.shift().replace(/=/, " = ") : l).join("\r\n"));
    for (const plane of machine === "mill" ? ["XY", "XZ", "YZ"] : ["XZ"]) add(`geometry-${plane}`, geometryFixture(machine, plane, units).text);
    const p = new Program({machine, units, name: "O'Brien \\ Ω 😀", timestamp: "2026-09-28T00:00:00Z"});
    p.addSection({name: "切削", tool: {number: 1, offset: 1, description: "tool ' \\ α"}, workOffset: 1, start: [2, 0, 2], spindle: {mode: "rpm", speed: 600, clockwise: true}, coolant: "off"}).linear([1, 0, 1], {mode: "perMinute", value: 12.3456789012345});
    add("unicode", p.toSTEP());
    const parserOnly = {syntax: "passed", envelope: "passed", normalization: "passed", references: "passed", targetParse: "passed", targetProfile: "failed", agreement: "passed"};
    for (const entity of ["APPLICATION_CONTEXT", "CARTESIAN_POINT", "PRODUCT"]) add(`shape-${entity.toLowerCase()}`, text.replace(new RegExp(`(${entity}\\()'[^']*'`), "$142."), {...parserOnly, targetCode: "PROFILE_SHAPE"}, "Parser-only oracle does not enforce EXPRESS or profile field types.");
    add("truncated", text.slice(0, -20), {syntax: "failed", envelope: "not_checked", normalization: "not_checked", references: "not_checked", targetParse: "failed", targetProfile: "failed", agreement: "not_checked", targetCode: "INVALID_NEXTNC"});
    add("trailing-record", text + "#99999=APPLICATION_CONTEXT('after terminator');\n", {syntax: "passed", envelope: "failed", normalization: "not_checked", references: "not_checked", targetParse: "failed", targetProfile: "failed", agreement: "not_checked", targetCode: "INVALID_NEXTNC"}, "Pinned upstream parser ignores trailing records; adapter checks full consumption.");
    const invalid = {syntax: "passed", envelope: "passed", normalization: "failed", references: "not_checked", targetParse: "failed", targetProfile: "failed", agreement: "not_checked", targetCode: "INVALID_NEXTNC"};
    add("malformed-x2", text.replace("Application protocol for the exchange of CNC data", "\\X2\\123\\X0\\"), invalid, "Upstream preserves raw strings; independent decoder rejects malformed X2.");
    add("unsafe-id", text.replace(/#1\b/g, "#9007199254740993"), invalid, "Upstream numeric ID conversion is not safe-integer validation.");
    add("duplicate-id", text.replace("\nENDSEC;\nEND-ISO", "\n#1=APPLICATION_CONTEXT('duplicate');\nENDSEC;\nEND-ISO"), invalid, "Upstream syntax parsing does not enforce ID uniqueness.");
    add("dangling-reference", text.replace(/(ACTION_PROPERTY_REPRESENTATION\('','',)#\d+/, "$1#99999"), {...invalid, normalization: "passed", references: "failed"}, "Upstream syntax parsing does not resolve references.");
  }
  return result;
}
module.exports = {cases};
