"use strict";
const fs = require("node:fs"), path = require("node:path"), {createHash} = require("node:crypto"), {spawnSync} = require("node:child_process");
process.chdir(__dirname);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
function files(dir) { return fs.readdirSync(dir, {withFileTypes: true}).flatMap(e => e.isDirectory() ? files(`${dir}/${e.name}`) : [`${dir}/${e.name}`]).sort(); }
function hashes(dir) { return Object.fromEntries(files(dir).map(f => [f, hash(fs.readFileSync(f))])); }
const source = JSON.parse(fs.readFileSync("source.json"));
if (JSON.stringify(hashes("upstream")) !== JSON.stringify(source.files)) throw new Error("Pinned upstream source hashes do not match; review and repin before building.");
if (require("typescript/package.json").version !== "5.9.3") throw new Error("Unexpected compiler version");
const result = spawnSync(process.execPath, [require.resolve("typescript/bin/tsc"), "-p", "tsconfig.json"], {encoding: "utf8", timeout: 60000});
if (result.status !== 0 || result.error) throw new Error(result.error?.message || result.stdout + result.stderr);
const outputs = hashes("build");
const pinned = JSON.parse(fs.readFileSync("build-hashes.json"));
if (JSON.stringify(outputs) !== JSON.stringify(pinned)) throw new Error("Compiled parser hashes differ from the reviewed build.");
fs.writeFileSync("build-identity.json", JSON.stringify({source: source.commit, sourceManifestSHA256: hash(fs.readFileSync("source.json")), compiler: "typescript@5.9.3", compilerSHA256: hash(fs.readFileSync(require.resolve("typescript/lib/_tsc.js"))), node: process.version, platform: process.platform, configSHA256: hash(fs.readFileSync("tsconfig.json")), lockSHA256: hash(fs.readFileSync("package-lock.json")), buildScriptSHA256: hash(fs.readFileSync("build.js")), buildSHA256: hash(JSON.stringify(outputs)), outputs}, null, 2) + "\n");
console.log(`Verified and compiled ${Object.keys(source.files).length} pinned upstream files.`);
