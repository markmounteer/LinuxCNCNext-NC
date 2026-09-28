"use strict";
const fs = require("node:fs"), path = require("node:path"), crypto = require("node:crypto");
const {resolveConfiguration} = require("./configuration");
function directory() {
  return resolveConfiguration().diagnostics.path;
}
function save(report) {
  const dir = directory(); fs.mkdirSync(dir, {recursive: true});
  const name = new Date().toISOString().replace(/[:.]/g, "-") + "-" + crypto.randomBytes(4).toString("hex") + ".json";
  const record = {schema: "linuxcnc-next-nc/diagnostic/1", timeUTC: new Date().toISOString(), ...report};
  const text = JSON.stringify(record, null, 2) + "\n";
  fs.writeFileSync(path.join(dir, name), text, {flag: "wx"});
  for (const index of ["latest.json", ...(report.status === "failed" ? ["latest-error.json"] : [])]) {
    const tmp = path.join(dir, index + "." + crypto.randomBytes(4).toString("hex") + ".tmp");
    fs.writeFileSync(tmp, text, {flag: "wx"}); fs.renameSync(tmp, path.join(dir, index));
  }
  return path.join(dir, name);
}
module.exports = {save, directory};
