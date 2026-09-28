#!/usr/bin/env node
"use strict";
const fs = require("node:fs"), path = require("node:path"), crypto = require("node:crypto");
const {translate} = require("../src/translate"), {readProgram} = require("../src/profile"), {template} = require("../src/plan");
const {NextNCError} = require("../src/errors"), diagnostics = require("../src/diagnostics");
const {writeExclusive} = require("../src/files");
const version = require("../package.json").version;
const usage = "Usage: nextnc-linuxcnc inspect|plan-template|translate input.stpnc [--plan plan.json] [--output NEW_FILE]";
const args = process.argv.slice(2);
if (args.length === 1 && ["--help", "--version"].includes(args[0])) { console.log(args[0] === "--help" ? usage : version); }
else {
  let input, planPath, output, command, inputHash, planHash;
  function hash(bytes) { return crypto.createHash("sha256").update(bytes).digest("hex"); }
  function read(file, limit) { const stat = fs.statSync(file); if (!stat.isFile() || stat.size > limit) throw new NextNCError("INPUT_SIZE", `Expected a regular file of at most ${limit} bytes: ${file}`); return fs.readFileSync(file); }
  function archive(report) {
    try { return diagnostics.save({translator: version, command, input, inputSHA256: inputHash, plan: planPath, planSHA256: planHash, ...report}); }
    catch (error) { console.error("Could not save diagnostics: " + error.message); return null; }
  }
  try {
    command = args.shift();
    if (!["inspect", "plan-template", "translate"].includes(command)) throw new NextNCError("USAGE", usage);
    while (args.length) {
      const arg = args.shift();
      if (arg === "--plan" && !planPath && args[0] && !args[0].startsWith("--")) planPath = path.resolve(args.shift());
      else if (arg === "--output" && !output && args[0] && !args[0].startsWith("--")) output = path.resolve(args.shift());
      else if (!arg.startsWith("--") && !input) input = path.resolve(arg);
      else throw new NextNCError("USAGE", usage);
    }
    if (!input || (command !== "translate" && planPath)) throw new NextNCError("USAGE", usage);
    const bytes = read(input, 32 * 1024 * 1024); inputHash = hash(bytes); const text = bytes.toString("utf8");
    let content, report;
    if (command === "translate") {
      if (!planPath) throw new NextNCError("PLAN_REQUIRED", "Supply --plan with reviewed tool/WCS mappings and entry/retract paths. Run plan-template to create an intentionally incomplete template.");
      const planBytes = read(planPath, 1024 * 1024); planHash = hash(planBytes);
      let plan; try { plan = JSON.parse(planBytes); } catch (error) { throw new NextNCError("PLAN_JSON", error.message); }
      const result = translate(text, plan); content = result.gcode; report = {inspection: result.report, sourceMap: result.sourceMap};
    } else {
      const program = readProgram(text); report = {inspection: program.report};
      content = JSON.stringify(command === "inspect" ? program.report : template(program), null, 2) + "\n";
    }
    // Everything has been validated before a file or stdout receives any code.
    if (output) writeExclusive(output, content);
    const archivePath = archive({status: "succeeded", output: output || "stdout", outputSHA256: hash(content), ...report});
    if (!output) process.stdout.write(content);
    if (archivePath) console.error("Next-NC report: " + archivePath);
  } catch (error) {
    const record = {code: error.code || "IO_ERROR", message: error.message, context: error.context || {}};
    const archivePath = archive({status: "failed", error: record});
    console.error(`Next-NC ${record.code}: ${record.message}`);
    if (Object.keys(record.context).length) console.error(JSON.stringify(record.context));
    if (archivePath) console.error("Next-NC report: " + archivePath);
    process.exitCode = error.code === "USAGE" ? 2 : 1;
  }
}
