"use strict";
const fs = require("node:fs"), path = require("node:path");
const {resolveConfiguration} = require("./configuration");
function doctor(configuration = resolveConfiguration()) {
  const checks = [];
  function check(name, fn) {
    try { checks.push({name, status: "passed", ...fn()}); }
    catch (error) { checks.push({name, status: "failed", code: error.code || "CONFIGURATION", message: error.message}); }
  }
  check("Node.js", () => {
    if (Number(process.versions.node.split(".")[0]) < 20) throw new Error("Node.js 20 or newer is required.");
    return {version: process.versions.node, executable: process.execPath};
  });
  for (const [key, label] of [["plan", "Execution plan"], ["toolTable", "Tool-table snapshot"]]) {
    const file = configuration[key].path;
    if (!file) { checks.push({name: label, status: "not_checked", message: "Optional snapshot is not configured."}); continue; }
    check(label, () => {
      const stat = fs.statSync(file);
      if (!stat.isFile() || stat.size > 1024 * 1024) throw new Error("Expected a regular file no larger than 1 MiB: " + file);
      fs.accessSync(file, fs.constants.R_OK);
      if (key === "plan") {
        const plan = JSON.parse(fs.readFileSync(file, "utf8"));
        if (!plan || !/^linuxcnc-next-nc\/execution-plan\/[1-4]$/.test(plan.schema)) throw new Error("Unrecognized execution-plan schema.");
      }
      return {path: file, bytes: stat.size, message: "Readable; run preflight with the actual program to validate contents."};
    });
  }
  check("Diagnostics directory", () => {
    let ancestor = configuration.diagnostics.path;
    while (!fs.existsSync(ancestor)) {
      const parent = path.dirname(ancestor);
      if (parent === ancestor) throw new Error("No existing diagnostics parent directory.");
      ancestor = parent;
    }
    if (!fs.statSync(ancestor).isDirectory()) throw new Error("Diagnostics path or its parent is not a directory: " + ancestor);
    fs.accessSync(ancestor, fs.constants.W_OK | fs.constants.X_OK);
    return {path: configuration.diagnostics.path, checkedAncestor: ancestor, message: "Filesystem access check only; no files or directories were created."};
  });
  return {schema: "linuxcnc-next-nc/doctor/1", status: checks.some(c => c.status === "failed") ? "failed" : "passed", configuration, checks,
    scope: "Read-only installation checks. Does not validate a job, LinuxCNC runtime, homing, limits, offsets, remaps or machine motion."};
}
module.exports = {doctor};
