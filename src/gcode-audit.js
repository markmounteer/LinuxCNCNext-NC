"use strict";
const {invariant} = require("./internal-error");
// Independent decoder of our emitted dialect, not an RS274 interpreter. Do not
// import the writer or its formatting helpers: these are the objects we audit.
const planeCode = {XY: 17, XZ: 18, YZ: 19};
function auditGcode(gcode, sourceMap, {machine, units}) {
  const check = (ok, rule, message, context = {}) => invariant(ok, "SERIALIZATION_" + rule, message, context);
  check(typeof gcode === "string" && gcode.endsWith("\n"), "TEXT", "Final G-code must end in a newline.");
  let offset = 0, count = 0, comments = 0, ended = false, initialized = false, plane = null;
  while (offset < gcode.length) {
    const end = gcode.indexOf("\n", offset), block = gcode.slice(offset, end), entry = sourceMap[count], c = entry?.command;
    const context = {line: count + 1, operation: entry?.operation, section: entry?.section, path: entry?.path,
      provenance: entry?.provenance, expectedCommand: c, observedBlock: block};
    check(end !== -1 && block.length > 0 && block.length <= 240, "BLOCK", "Invalid final G-code block.", context);
    check(c && entry.line === count + 1 && !ended, "ASSOCIATION", "Final block has no matching command or follows program end.", context);
    if (c.type === "comment") {
      // Compare each UTF-16 character independently with the documented ASCII
      // replacement/truncation contract, including both halves of surrogates.
      check(typeof c.text === "string" && block.length === Math.min(c.text.length, 150) + 2 && block[0] === "(" && block.at(-1) === ")",
        "COMMENT", "Comment does not match the passive comment contract.", context);
      for (let i = 0; i < block.length - 2; i++) {
        const ch = c.text[i], n = c.text.charCodeAt(i), safe = n >= 32 && n <= 126 && !"();%".includes(ch);
        check(block[i + 1] === (safe ? ch : "_"), "COMMENT", "Comment text differs from the sanitized source.", context);
      }
      // Allow only the translator's passive prefixes, including those applied
      // before user names. This also excludes active directives using spaces
      // rather than commas, without maintaining a partial directive blacklist.
      check(/^\((?:LinuxCNCNext-NC |Program fingerprint |Program: |Section [1-9][0-9]*: )/.test(block),
        "COMMENT_PREFIX", "Comment lacks a translator-owned passive prefix.", context);
      comments++;
    } else {
      const observed = Object.create(null);
      for (const token of block.split(" ")) {
        const match = /^([A-Z$])(-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?)$/.exec(token);
        check(match && Number.isFinite(Number(match[2])), "TOKEN", "Invalid numeric word or unexpected executable text.", {...context, observedToken: token});
        (observed[match[1]] ||= []).push(Number(match[2]));
      }
      context.observedWords = observed;
      const expected = Object.create(null), word = (address, value) => { (expected[address] ||= []).push(value); };
      const axes = (values, center = false) => {
        for (const [axis, value] of Object.entries(values)) {
          check((machine === "mill" ? ["x", "y", "z"] : ["x", "z"]).includes(axis), "AXIS", "Unsupported serialized axis.", context);
          word(center ? {x: "I", y: "J", z: "K"}[axis] : axis.toUpperCase(), value);
        }
      };
      check(c.type === "initialize" || initialized, "STATE", "Serialized command requires initialization after tool change.", context);
      switch (c.type) {
        case "initialize":
          check(c.units === units && (machine === "mill" || c.plane === "XZ"), "STATE", "Initialization disagrees with the program context.", context);
          for (const value of [units === "mm" ? 21 : 20, planeCode[c.plane], 8, 90, 91.1, 40, 80, 94, 61]) word("G", value);
          initialized = true; plane = c.plane; break;
        case "clearTemporaryOffsets": word("G", 92.1); break;
        case "spindleStop": word("M", 5); word("$", 0); break;
        case "spindleMode": word("G", c.mode === "css" ? 96 : 97); word("S", c.speed); word("$", 0); if (c.mode === "css") word("D", c.maximumRPM); break;
        case "spindleStart": word("M", c.clockwise ? 3 : 4); word("$", 0); break;
        case "coolant": word("M", {off: 9, flood: 8, mist: 7}[c.value]); break;
        case "feed": word("G", c.mode === "perRevolution" ? 95 : 94); if (c.value !== undefined) word("F", c.value); break;
        case "cancelToolOffset": word("G", 49); break;
        case "toolChange": word("T", c.tool); word("M", 6); initialized = false; plane = null; break;
        case "workOffset": word("G", Number(c.value.slice(1))); break;
        case "toolOffset": word("G", 43); word("H", c.offset); break;
        case "plane": word("G", planeCode[c.value]); plane = c.value; break;
        case "rapid": case "linear": if (c.frame === "machine") word("G", 53); word("G", c.type === "rapid" ? 0 : 1); axes(c.axes); break;
        case "arc":
          check(plane === c.plane, "PLANE", "Arc disagrees with the serialized active plane.", context);
          word("G", c.clockwise ? 2 : 3); axes(c.axes); axes(c.centerOffset, true); if (c.fullCircle) word("P", 1); break;
        case "dwell": word("G", 4); word("P", c.seconds); break;
        case "end": word("M", 2); ended = true; break;
        default: check(false, "COMMAND", "Unsupported command in final serialization audit.", context);
      }
      // Match exact numeric multisets. Several distinct G words are legitimate
      // in initialization; repeated addresses/extra codes must not disappear.
      check(Object.keys(observed).length === Object.keys(expected).length && Object.entries(expected).every(([key, values]) => {
        const actual = observed[key];
        values.sort((a, b) => a - b);
        return actual?.length === values.length && values.every(v => Number.isFinite(v)) &&
          actual.sort((a, b) => a - b).every((v, i) => v === values[i]);
      }), "WORDS", "Final G-code words disagree with the validated command.", context);
    }
    count++; offset = end + 1;
  }
  check(count === sourceMap.length && ended, "END", "Final G-code does not contain every command and exactly one terminating M2.", {line: count, expectedLines: sourceMap.length});
  return {schema: "linuxcnc-next-nc/serialization-audit/1", status: "passed", checkedLines: count, executableBlocks: count - comments, comments,
    scope: "Final emitted text matches validated commands; not arbitrary RS274 interpretation or controller/machine acceptance."};
}
module.exports = {auditGcode};
