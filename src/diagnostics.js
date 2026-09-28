"use strict";
const fs = require("node:fs"), path = require("node:path"), os = require("node:os"), crypto = require("node:crypto");
function directory() {
  if (process.env.NEXTNC_DIAGNOSTICS) return path.resolve(process.env.NEXTNC_DIAGNOSTICS);
  const root = process.platform === "win32" ? process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local") : process.env.XDG_STATE_HOME || path.join(os.homedir(), ".local", "state");
  return path.join(root, "LinuxCNCNext-NC", "diagnostics");
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
