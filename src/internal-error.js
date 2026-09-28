"use strict";
// A translator invariant is not a CAM or execution-plan validation failure.
class InternalError extends Error {
  constructor(invariant, message, context = {}) {
    super(message); this.name = "InternalError"; this.code = "INTERNAL_ERROR";
    this.context = {...context, stage: "translation", invariant};
  }
}
function invariant(ok, rule, message, context) { if (!ok) throw new InternalError(rule, message, context); }
module.exports = {InternalError, invariant};
