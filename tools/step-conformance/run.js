"use strict";
// A separate, memory-limited process bounds any upstream parser failure/hang.
const {spawnSync} = require("node:child_process"), fs = require("node:fs"), path = require("node:path");
const dir = path.resolve(__dirname, "../../artifacts/step-conformance"); fs.mkdirSync(dir, {recursive: true});
const run = spawnSync(process.execPath, ["--max-old-space-size=512", path.join(__dirname, "compare.js")], {encoding: "utf8", timeout: 60000, maxBuffer: 4 * 1024 * 1024});
fs.writeFileSync(path.join(dir, "runner.json"), JSON.stringify({status: run.status, signal: run.signal, error: run.error?.message, stdout: run.stdout, stderr: run.stderr}, null, 2) + "\n");
process.stdout.write(run.stdout || ""); process.stderr.write(run.stderr || "");
if (run.error) console.error(run.error.message);
process.exitCode = run.status === 0 && !run.error ? 0 : 1;
