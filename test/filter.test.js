"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), fs = require("node:fs"), os = require("node:os"), path = require("node:path");
const {spawnSync} = require("node:child_process"), {example, millingExample} = require("../scripts/example");
const {translate} = require("../src/translate");
test("real POSIX filter handles default/override paths, both machines and missing runtime without stdout pollution", {skip: process.platform === "win32"}, () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc shell "));
  try {
    const wrapper = path.resolve(__dirname, "../bin/nextnc-filter"), config = path.join(dir, "configuration space"), planFile = path.join(config, "LinuxCNCNext-NC", "plan.json");
    fs.mkdirSync(path.dirname(planFile), {recursive: true});
    const env = {...process.env, NEXTNC_PLAN: "", NEXTNC_TOOL_TABLE: "", XDG_CONFIG_HOME: config, NEXTNC_DIAGNOSTICS: path.join(dir, "diagnostics")};
    for (const fixture of [example, millingExample]) {
      const {text, plan} = fixture(), input = path.join(dir, "part ' quoted.stpnc");
      fs.writeFileSync(input, text); fs.writeFileSync(planFile, JSON.stringify(plan));
      let run = spawnSync("/bin/sh", [wrapper, input], {encoding: "utf8", env});
      assert.equal(run.status, 0, run.stderr); assert.equal(run.stdout, translate(text, plan).gcode);
      run = spawnSync("/bin/sh", [wrapper, input], {encoding: "utf8", env: {...env, NEXTNC_PLAN: path.join(dir, "missing.json")}});
      assert.equal(run.status, 1); assert.equal(run.stdout, "");
    }
    const tools = path.join(dir, "without node"); fs.mkdirSync(tools);
    fs.symlinkSync("/usr/bin/dirname", path.join(tools, "dirname"));
    const missing = spawnSync("/bin/sh", [wrapper, "missing.stpnc"], {encoding: "utf8", env: {...env, PATH: tools}});
    assert.equal(missing.status, 127); assert.equal(missing.stdout, ""); assert.match(missing.stderr, /NODE_MISSING/);
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
