"use strict";
const path = require("node:path"), os = require("node:os");
function resolveConfiguration(env = process.env, platform = process.platform, home = os.homedir()) {
  const configRoot = env.XDG_CONFIG_HOME || path.join(env.HOME || home, ".config");
  const stateRoot = platform === "win32" ? env.LOCALAPPDATA || path.join(home, "AppData", "Local") : env.XDG_STATE_HOME || path.join(env.HOME || home, ".local", "state");
  return {
    plan: {path: path.resolve(env.NEXTNC_PLAN || path.join(configRoot, "LinuxCNCNext-NC", "plan.json")), source: env.NEXTNC_PLAN ? "NEXTNC_PLAN" : "XDG_CONFIG_HOME or home default"},
    toolTable: {path: env.NEXTNC_TOOL_TABLE ? path.resolve(env.NEXTNC_TOOL_TABLE) : null, source: env.NEXTNC_TOOL_TABLE ? "NEXTNC_TOOL_TABLE" : "not configured"},
    diagnostics: {path: path.resolve(env.NEXTNC_DIAGNOSTICS || path.join(stateRoot, "LinuxCNCNext-NC", "diagnostics")), source: env.NEXTNC_DIAGNOSTICS ? "NEXTNC_DIAGNOSTICS" : "platform state directory"}
  };
}
module.exports = {resolveConfiguration};
