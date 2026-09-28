"use strict";
const fs = require("node:fs"), path = require("node:path"), crypto = require("node:crypto");
function writeExclusive(output, content) {
  // Stage in the destination directory, then create the final name atomically
  // without replacing an existing file. A failed write never leaves a partial
  // program at the requested path. Both Linux and NTFS support hard links.
  const tmp = path.join(path.dirname(output), ".nextnc-" + crypto.randomBytes(12).toString("hex") + ".tmp");
  try { fs.writeFileSync(tmp, content, {flag: "wx"}); fs.linkSync(tmp, output); }
  finally { if (fs.existsSync(tmp)) fs.unlinkSync(tmp); }
}
module.exports = {writeExclusive};
