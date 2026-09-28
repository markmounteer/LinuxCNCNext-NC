"use strict";
const scopes = {
  part21: "Bounded emitted Part 21 syntax, header, record IDs and reference existence",
  profileShape: "Accepted Next-NC entity components, attribute shapes and reference target types",
  semantics: "Supported geometry, units, process state and toolpath semantics",
  executionPlan: "Reviewed plan binding, mappings and transitions",
  toolTable: "Optional file snapshot syntax and mapped T/H record presence",
  completeness: "Candidate command generation and ordered execution completeness",
  serialization: "Final candidate G-code text against audited command records",
  expressSchema: "General EXPRESS schema parse/build and instance loading",
  expressAttributes: "General EXPRESS attribute and SELECT type conformance",
  entityWhere: "General EXPRESS entity WHERE rules",
  typeWhere: "General EXPRESS defined-type WHERE rules",
  uniqueness: "General EXPRESS uniqueness constraints",
  globalRules: "General EXPRESS global RULE evaluation",
  fullAP238: "Full AP238 conformance beyond the two supported toolpath profiles"
};
function coverageTracker(initial) {
  const coverage = initial ? structuredClone(initial) : {
    schema: "linuxcnc-next-nc/validation-coverage/1",
    stages: Object.fromEntries(Object.entries(scopes).map(([name, scope]) => [name, {status: "not_checked", scope}]))
  };
  let current;
  return {
    coverage,
    start(name) { current = name; },
    pass(evidence = {}) { coverage.stages[current] = {...coverage.stages[current], ...evidence, status: "passed"}; current = undefined; },
    skip(evidence) { coverage.stages[current] = {...coverage.stages[current], ...evidence, status: "not_checked"}; current = undefined; },
    fail(error) {
      if (current) coverage.stages[current] = {...coverage.stages[current], status: "failed", code: error.code || "INTERNAL_ERROR"};
      error.validationCoverage = coverage;
    }
  };
}
module.exports = {coverageTracker};
