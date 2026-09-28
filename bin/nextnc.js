#!/usr/bin/env node
"use strict";
const fs = require("node:fs"), path = require("node:path"), crypto = require("node:crypto");
const {translate} = require("../src/translate"), {readProgram} = require("../src/profile"), {template} = require("../src/plan");
const {NextNCError} = require("../src/errors"), diagnostics = require("../src/diagnostics");
const {writeExclusive} = require("../src/files");
const {checkArtifact} = require("../src/artifact-identity");
const {resolveConfiguration} = require("../src/configuration"), {doctor} = require("../src/doctor"), {renderReport} = require("../src/report");
const version = require("../package.json").version;
const usage = "Usage: nextnc-linuxcnc inspect|plan-template|preflight|translate input.stpnc [--plan plan.json] [--tool-table tool.tbl] [--output NEW_FILE]\n       nextnc-linuxcnc report diagnostic.json [--gcode job.ngc] --output NEW_REPORT.html\n       nextnc-linuxcnc doctor [--output NEW_FILE.json]";
const args = process.argv.slice(2);
if (args.length === 1 && ["--help", "--version"].includes(args[0])) { console.log(args[0] === "--help" ? usage : version); }
else {
  let input, planPath, toolTablePath, output, command, inputHash, planHash, toolTableHash, gcodePath;
  function hash(bytes) { return crypto.createHash("sha256").update(bytes).digest("hex"); }
  function read(file, limit) { const stat = fs.statSync(file); if (!stat.isFile() || stat.size > limit) throw new NextNCError("INPUT_SIZE", `Expected a regular file of at most ${limit} bytes: ${file}`); return fs.readFileSync(file); }
  function archive(report) {
    if (["report", "doctor"].includes(command)) return null;
    try { return diagnostics.save({translator: version, command, input, inputSHA256: inputHash, plan: planPath, planSHA256: planHash, toolTable: toolTablePath, toolTableSHA256: toolTableHash, ...report}); }
    catch (error) { console.error("Could not save diagnostics: " + error.message); return null; }
  }
  try {
    command = args.shift();
    if (!["inspect", "plan-template", "preflight", "translate", "report", "doctor", "filter"].includes(command)) throw new NextNCError("USAGE", usage);
    while (args.length) {
      const arg = args.shift();
      if (arg === "--plan" && !planPath && args[0] && !args[0].startsWith("--")) planPath = path.resolve(args.shift());
      else if (arg === "--tool-table" && !toolTablePath && args[0] && !args[0].startsWith("--")) toolTablePath = path.resolve(args.shift());
      else if (arg === "--output" && !output && args[0] && !args[0].startsWith("--")) output = path.resolve(args.shift());
      else if (arg === "--gcode" && !gcodePath && args[0] && !args[0].startsWith("--")) gcodePath = path.resolve(args.shift());
      else if (!arg.startsWith("--") && !input) input = path.resolve(arg);
      else throw new NextNCError("USAGE", usage);
    }
    if (gcodePath && command !== "report") throw new NextNCError("USAGE", "--gcode is only accepted by report.");
    if (command === "filter") {
      if (!input || planPath || toolTablePath || output) throw new NextNCError("USAGE", "The LinuxCNC filter accepts only the input filename; configure NEXTNC_PLAN and optionally NEXTNC_TOOL_TABLE.");
      const config = resolveConfiguration(); planPath = config.plan.path; toolTablePath = config.toolTable.path;
    }
    if (command === "doctor") {
      if (input || planPath || toolTablePath) throw new NextNCError("USAGE", usage);
      const result = doctor(), content = JSON.stringify(result, null, 2) + "\n";
      if (output) writeExclusive(output, content); else process.stdout.write(content);
      if (result.status === "failed") process.exitCode = 1;
    } else if (command === "report") {
      if (!input || !output || planPath || toolTablePath) throw new NextNCError("USAGE", usage);
      let record; try { record = JSON.parse(read(input, 64 * 1024 * 1024)); } catch (error) { if (error instanceof SyntaxError) throw new NextNCError("REPORT_JSON", error.message); throw error; }
      const artifactIdentity = checkArtifact(record, gcodePath);
      writeExclusive(output, renderReport(record, {artifactIdentity}));
      console.error("Next-NC review: " + output);
      if (gcodePath) {
        console.error("Next-NC saved G-code identity: " + artifactIdentity.status);
        if (artifactIdentity.status !== "match") process.exitCode = 1;
      }
    } else {
    if (!input || (!["preflight", "translate", "filter"].includes(command) && (planPath || toolTablePath))) throw new NextNCError("USAGE", usage);
    const bytes = read(input, 32 * 1024 * 1024); inputHash = hash(bytes); const text = bytes.toString("utf8");
    let content, report;
    if (["translate", "preflight", "filter"].includes(command)) {
      if (!planPath) throw new NextNCError("PLAN_REQUIRED", "Supply --plan with reviewed tool/WCS mappings and entry/retract paths. Run plan-template to create an intentionally incomplete template.");
      const planBytes = read(planPath, 1024 * 1024); planHash = hash(planBytes);
      let plan; try { plan = JSON.parse(planBytes); } catch (error) { throw new NextNCError("PLAN_JSON", error.message); }
      let toolTable;
      if (toolTablePath) { const tableBytes = read(toolTablePath, 1024 * 1024); toolTableHash = hash(tableBytes); toolTable = tableBytes.toString("utf8"); }
      const result = translate(text, plan, {toolTable}); report = {inspection: result.report, sourceMap: result.sourceMap};
      content = command !== "preflight" ? result.gcode : JSON.stringify({status: "passed", gcodeWritten: false, scope: "Offline program, execution-plan and optional tool-table validation; not machine acceptance.", ...report}, null, 2) + "\n";
    } else {
      const program = readProgram(text); report = {inspection: program.report};
      content = JSON.stringify(command === "inspect" ? program.report : template(program), null, 2) + "\n";
    }
    // Everything has been validated before a file or stdout receives any code.
    if (output) writeExclusive(output, content);
    const archivePath = archive({status: "succeeded", output: output || "stdout", outputSHA256: hash(content), ...report});
    if (!output) process.stdout.write(content);
    if (archivePath) console.error("Next-NC report: " + archivePath);
    }
  } catch (error) {
    const record = {code: error.code || "INTERNAL_ERROR", message: error.message, context: error.context || {}};
    const archivePath = archive({status: "failed", error: record, ...(error.validationCoverage ? {inspection: {validationCoverage: error.validationCoverage}} : {})});
    console.error(`Next-NC ${record.code}: ${record.message}`);
    if (Object.keys(record.context).length) console.error(JSON.stringify(record.context));
    if (archivePath) console.error("Next-NC report: " + archivePath);
    process.exitCode = error.code === "USAGE" ? 2 : 1;
  }
}
