"use strict";
class NextNCError extends Error {
  constructor(code, message, context = {}) { super(message); this.name = "NextNCError"; this.code = code; this.context = context; }
}
function requireValue(ok, code, message, context) { if (!ok) throw new NextNCError(code, message, context); }
function correctionFor(error) {
  if (!error) return undefined;
  if (error.context?.stage === "geometry") return "Review the named Fusion operation and its source geometry, regenerate its toolpath and post again. The reported threshold is the existing format check; changing the execution plan cannot repair this geometry.";
  if (/^TOOL_TABLE/.test(error.code)) return "Check the named T/H records in the existing LinuxCNC tool-table snapshot and the reviewed plan mapping.";
  if (error.context?.field || error.context?.issues) return "Correct the named execution-plan fields and rerun preflight. Preserve the intended tool, work offset and reviewed transition path.";
  if (error.code === "INVALID_NEXTNC" || error.code === "PROFILE") return "Regenerate the Next-NC file with the matching Fusion post. Include this report and the source record when reporting a format problem.";
  if (error.code === "INTERNAL_ERROR") return "Report this implementation failure with the diagnostic archive; do not change machining geometry to bypass it.";
  return "Review the error message and its recorded source or configuration field, then rerun preflight.";
}
module.exports = {NextNCError, requireValue, correctionFor};
