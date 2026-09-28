"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), fs = require("node:fs"), os = require("node:os"), path = require("node:path"), crypto = require("node:crypto");
const {spawnSync} = require("node:child_process"), {translate} = require("../src/translate");
const {semanticFixture} = require("./support/semantic-fixture"), {checkArtifact, MAX_GCODE_BYTES} = require("../src/artifact-identity"), {renderReport} = require("../src/report");
const hash = bytes => crypto.createHash("sha256").update(bytes).digest("hex");
test("saved-file identity uses exact candidate bytes for both machines/units and distrusts missing hashes", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-identity-")), file = path.join(dir, "same-job.ngc");
  try {
    for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
      const f = semanticFixture(machine, units), out = translate(f.text, f.plan);
      const record = {schema: "linuxcnc-next-nc/diagnostic/1", outputSHA256: hash("preflight JSON"), inspection: out.report};
      const before = JSON.stringify(record); fs.writeFileSync(file, out.gcode);
      const match = checkArtifact(record, file);
      assert.equal(match.status, "match"); assert.equal(match.bytes, Buffer.byteLength(out.gcode)); assert.equal(match.observedSHA256, out.report.traceability.gcodeSHA256);
      assert.ok(!isNaN(Date.parse(match.checkedUTC))); assert.equal(JSON.stringify(record), before);
      for (const changed of [out.gcode.replace("F0.2", "F0.3"), out.gcode.replaceAll("\n", "\r\n"), "\uFEFF" + out.gcode, out.gcode + "(extra comment)\n", out.gcode.replace("X4", "X5")]) {
        fs.writeFileSync(file, changed); assert.equal(checkArtifact(record, file).status, "mismatch");
      }
      fs.writeFileSync(file, out.gcode);
      for (const [expected, status] of [[undefined, "not_recorded"], [null, "not_recorded"], ["", "invalid_hash"], ["a".repeat(63), "invalid_hash"], [{hash: match.observedSHA256}, "invalid_hash"], [match.observedSHA256.toUpperCase(), "match"]]) {
        const legacy = {schema: record.schema, outputSHA256: match.observedSHA256, inspection: {traceability: {gcodeSHA256: expected}}};
        assert.equal(checkArtifact(legacy, file).status, status);
      }
      const hostile = {...record, output: path.join(dir, "must-not-read"), artifactIdentity: match};
      assert.equal(checkArtifact(hostile).status, "not_checked");
      assert.match(renderReport(hostile), /Saved G-code identity: Not checked/);
      const html = renderReport(record, {artifactIdentity: {...match, file: '<script>alert("x")</script>'}});
      assert.match(html, /Saved G-code identity: Match/); assert.doesNotMatch(html, /<script/i); assert.match(html, /&lt;script&gt;/);
    }
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
test("identity bounds actual reads, rejects non-files and detects files changing during the check", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-identity-size-")), file = path.join(dir, "large.ngc");
  const fstat = fs.fstatSync, read = fs.readSync;
  try {
    assert.throws(() => checkArtifact({}, path.join(dir, "missing")), e => e.code === "ENOENT");
    assert.throws(() => checkArtifact({}, dir));
    const fd = fs.openSync(file, "w"); try { fs.ftruncateSync(fd, MAX_GCODE_BYTES + 1); } finally { fs.closeSync(fd); }
    assert.throws(() => checkArtifact({}, file), e => e.code === "INPUT_SIZE");
    let bytesRead = 0;
    fs.fstatSync = fd => { const stat = fstat(fd); stat.size = 0; return stat; };
    fs.readSync = (...args) => { const n = read(...args); bytesRead += n; return n; };
    assert.throws(() => checkArtifact({}, file), e => e.code === "INPUT_SIZE");
    assert.equal(bytesRead, MAX_GCODE_BYTES + 1);
    fs.readSync = read; fs.fstatSync = fstat;
    fs.writeFileSync(file, "M2\n"); let calls = 0;
    fs.fstatSync = fd => { const stat = fstat(fd); if (++calls > 1) stat.mtimeMs += 1; return stat; };
    assert.throws(() => checkArtifact({}, file), e => e.code === "ARTIFACT_CHANGED");
    fs.fstatSync = fstat;
    if (process.platform !== "win32") {
      const fifo = path.join(dir, "pipe"); const created = spawnSync("mkfifo", [fifo]); assert.equal(created.status, 0);
      assert.throws(() => checkArtifact({}, fifo), e => e.code === "INPUT_SIZE");
    }
  } finally { fs.fstatSync = fstat; fs.readSync = read; fs.rmSync(dir, {recursive: true, force: true}); }
});
test("CLI reports match translation/preflight candidates and preserve archives on mismatches and errors", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "nextnc-identity-cli-"));
  try {
    const input = path.join(dir, "part.stpnc"), plan = path.join(dir, "plan.json"), file = path.join(dir, "part.ngc"), diagnostics = path.join(dir, "diagnostics"), latest = path.join(diagnostics, "latest.json");
    const run = (...args) => spawnSync(process.execPath, [path.join(__dirname, "../bin/nextnc.js"), ...args], {encoding: "utf8", env: {...process.env, NEXTNC_DIAGNOSTICS: diagnostics}});
    const snapshot = () => Object.fromEntries(fs.readdirSync(diagnostics).map(name => [name, hash(fs.readFileSync(path.join(diagnostics, name)))]));
    let sequence = 0;
    const review = (archive, extra = []) => {
      const dest = path.join(dir, `report-${++sequence}.html`), before = snapshot();
      const r = run("report", archive, ...extra, "--output", dest);
      assert.equal(r.stdout, ""); assert.deepEqual(snapshot(), before); return {r, dest};
    };
    for (const machine of ["lathe", "mill"]) for (const units of ["mm", "inch"]) {
      const f = semanticFixture(machine, units); fs.writeFileSync(input, f.text); fs.writeFileSync(plan, JSON.stringify(f.plan));
      for (const command of ["translate", "preflight"]) {
        const translated = run(command, input, "--plan", plan); assert.equal(translated.status, 0, translated.stderr);
        if (command === "translate") fs.writeFileSync(file, translated.stdout);
        else assert.notEqual(JSON.parse(fs.readFileSync(latest)).outputSHA256, hash(fs.readFileSync(file)));
        const {r, dest} = review(latest, ["--gcode", file]); assert.equal(r.status, 0, r.stderr); assert.match(fs.readFileSync(dest, "utf8"), /Saved G-code identity: Match/);
        const untouched = fs.readFileSync(dest), before = snapshot();
        assert.equal(run("report", latest, "--gcode", file, "--output", dest).status, 1); assert.deepEqual(fs.readFileSync(dest), untouched); assert.deepEqual(snapshot(), before);
      }
      fs.appendFileSync(file, "(edited)\n");
      let result = review(latest, ["--gcode", file]); assert.equal(result.r.status, 1); assert.match(fs.readFileSync(result.dest, "utf8"), /Saved G-code identity: Mismatch/);
      const legacy = JSON.parse(fs.readFileSync(latest)); delete legacy.inspection.traceability.gcodeSHA256;
      legacy.output = path.join(dir, "nonexistent-file"); legacy.outputSHA256 = hash(fs.readFileSync(file));
      const archive = path.join(dir, "legacy.json"); fs.writeFileSync(archive, JSON.stringify(legacy));
      result = review(archive, ["--gcode", file]); assert.equal(result.r.status, 1); assert.match(fs.readFileSync(result.dest, "utf8"), /Saved G-code identity: Not recorded/);
      result = review(archive); assert.equal(result.r.status, 0); assert.match(fs.readFileSync(result.dest, "utf8"), /Saved G-code identity: Not checked/);
      result = review(archive, ["--gcode", legacy.output]); assert.equal(result.r.status, 1); assert.ok(!fs.existsSync(result.dest));
      result = review(archive, ["--gcode", dir]); assert.equal(result.r.status, 1); assert.ok(!fs.existsSync(result.dest));
      assert.equal(fs.readFileSync(input, "utf8"), f.text); assert.deepEqual(JSON.parse(fs.readFileSync(plan)), f.plan);
      assert.equal(fs.readFileSync(archive, "utf8"), JSON.stringify(legacy));
      assert.ok(fs.readFileSync(file, "utf8").endsWith("(edited)\n"));
    }
    assert.equal(run("report", latest, "--gcode").status, 2);
    assert.equal(run("doctor", "--gcode", file).status, 2);
    assert.equal(run("report", latest, "--gcode", file, "--gcode", file).status, 2);
  } finally { fs.rmSync(dir, {recursive: true, force: true}); }
});
