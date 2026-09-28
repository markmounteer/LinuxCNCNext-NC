"use strict";
const fs = require("node:fs"), crypto = require("node:crypto");
const {NextNCError} = require("./errors");
const MAX_GCODE_BYTES = 64 * 1024 * 1024;
function checkArtifact(record, file) {
  if (!file) return {schema: "linuxcnc-next-nc/artifact-identity/1", status: "not_checked"};
  // Open once and inspect the handle. Nonblocking open lets us reject POSIX
  // pipes without waiting for a writer; no archive-provided path is opened.
  const fd = fs.openSync(file, fs.constants.O_RDONLY | (fs.constants.O_NONBLOCK || 0));
  const hash = crypto.createHash("sha256"); let bytes = 0;
  try {
    const before = fs.fstatSync(fd);
    if (!before.isFile() || before.size > MAX_GCODE_BYTES) throw new NextNCError("INPUT_SIZE", `Expected a regular G-code file of at most ${MAX_GCODE_BYTES} bytes: ${file}`);
    const buffer = Buffer.alloc(64 * 1024);
    for (;;) {
      const n = fs.readSync(fd, buffer, 0, Math.min(buffer.length, MAX_GCODE_BYTES - bytes + 1), null);
      if (!n) break;
      bytes += n;
      if (bytes > MAX_GCODE_BYTES) throw new NextNCError("INPUT_SIZE", `G-code exceeds ${MAX_GCODE_BYTES} bytes: ${file}`);
      hash.update(buffer.subarray(0, n));
    }
    const after = fs.fstatSync(fd);
    if (before.size !== after.size || before.mtimeMs !== after.mtimeMs || bytes !== after.size) throw new NextNCError("ARTIFACT_CHANGED", "Selected G-code changed while being read; repeat the identity check.");
  } finally { fs.closeSync(fd); }
  const observedSHA256 = hash.digest("hex"), expected = record?.inspection?.traceability?.gcodeSHA256;
  const valid = typeof expected === "string" && /^[a-f\d]{64}$/i.test(expected);
  const expectedSHA256 = valid ? expected.toLowerCase() : null;
  return {schema: "linuxcnc-next-nc/artifact-identity/1", status: valid ? (expectedSHA256 === observedSHA256 ? "match" : "mismatch") : expected == null ? "not_recorded" : "invalid_hash",
    file, bytes, expectedSHA256, observedSHA256, checkedUTC: new Date().toISOString(),
    scope: "Exact bytes read compared with the archived candidate hash; not authenticity, current controller loading or machine acceptance."};
}
module.exports = {checkArtifact, MAX_GCODE_BYTES};
