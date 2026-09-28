"use strict";
class NextNCError extends Error {
  constructor(code, message, context = {}) { super(message); this.name = "NextNCError"; this.code = code; this.context = context; }
}
function requireValue(ok, code, message, context) { if (!ok) throw new NextNCError(code, message, context); }
module.exports = {NextNCError, requireValue};
