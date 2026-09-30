"use strict";
const crypto = require("node:crypto");
const {requireValue: need} = require("./errors");
const {pointAt} = require("./review-geometry");
const finite = v => Number.isFinite(v) && Math.abs(v) < 1e9;
const keys = (v, allowed) => v && typeof v === "object" && !Array.isArray(v) && Object.keys(v).every(k => allowed.includes(k));
function stockPreview(review, setup) {
  const fail = (ok, message) => need(ok, "STOCK_PREVIEW", message);
  fail(keys(setup, ["schema", "machine", "units", "programFingerprint", "gcodeSHA256", "workOffset", "stock", "tools", "resolution"]), "Invalid stock-preview setup fields.");
  fail(setup.schema === "linuxcnc-next-nc/stock-setup/1" && setup.machine === "mill" && review.machine === "mill", "Stock preview currently supports fixed XYZ milling only.");
  fail(setup.units === review.units && ["mm", "inch"].includes(setup.units), "Stock and tool dimensions must use the recorded program units.");
  fail(/^[a-f0-9]{64}$/.test(setup.gcodeSHA256) && /^[a-f0-9]{64}$/.test(setup.programFingerprint) && setup.gcodeSHA256 === review.gcodeSHA256 && setup.programFingerprint === review.programFingerprint, "Stock setup must match both the archived program fingerprint and candidate G-code SHA-256.");
  fail(/^G5[4-9](\.[123])?$/.test(setup.workOffset) && !(setup.workOffset.includes(".") && !setup.workOffset.startsWith("G59.")), "Specify one valid G54..G59.3 stock work offset.");
  fail(keys(setup.stock, ["min", "max"]) && [setup.stock.min, setup.stock.max].every(p => Array.isArray(p) && p.length === 3 && p.every(finite)) && setup.stock.min.every((v, i) => v < setup.stock.max[i]), "Stock must be an explicit axis-aligned XYZ block with increasing finite bounds.");
  fail(finite(setup.resolution) && setup.resolution > 0, "Preview resolution must be positive in program units.");
  fail(setup.tools && keys(setup.tools, Object.keys(setup.tools)) && Object.keys(setup.tools).length > 0 && Object.keys(setup.tools).length <= 1000, "Provide the mapped LinuxCNC tool geometries.");
  for (const [id, tool] of Object.entries(setup.tools)) {
    fail(/^[1-9]\d*$/.test(id) && keys(tool, ["shape", "diameter", "cuttingLength"]) && tool.shape === "flat-end", "Only explicit flat-end mill geometries keyed by mapped T number are supported.");
    fail(finite(tool.diameter) && tool.diameter >= 2 * setup.resolution && finite(tool.cuttingLength) && tool.cuttingLength > 0, "Tool diameter must be at least twice the grid resolution; cuttingLength must be positive.");
  }
  const {min, max} = setup.stock, nx = Math.ceil((max[0] - min[0]) / setup.resolution), ny = Math.ceil((max[1] - min[1]) / setup.resolution);
  fail(nx <= 512 && ny <= 512 && nx * ny <= 262144, "Stock preview exceeds the 512 by 512 cell limit; increase the preview resolution.");
  const dx = (max[0] - min[0]) / nx, dy = (max[1] - min[1]) / ny;
  const heights = new Array(nx * ny).fill(max[2]), unresolvedLines = [], rapidIntersections = [];
  let visits = 0, samples = 0, cuttingBlocks = 0;
  for (const r of review.records) {
    if (!["rapid", "linear", "arc"].includes(r.kind)) continue;
    if (r.frame === "machine") { unresolvedLines.push({line: r.line, reason: "Machine-to-work transform is unknown"}); continue; }
    fail(r.frame === "work:" + setup.workOffset, "Multiple or unmatched work offsets need explicit transforms; they cannot share this stock preview.");
    const tool = setup.tools[String(r.state.tool)];
    fail(tool, `No preview geometry for mapped tool T${r.state.tool} at line ${r.line}.`);
    if (!r.drawable) { unresolvedLines.push({line: r.line, reason: "Motion has unknown starting coordinates"}); continue; }
    const radius = tool.diameter / 2, steps = Math.max(1, Math.ceil(r.distance / (Math.min(dx, dy) / 2)));
    fail(Number.isFinite(steps) && samples + steps + 1 <= 200000, "Stock preview exceeds 200000 path samples; shorten the review or increase resolution.");
    let intersection = false;
    if (r.kind !== "rapid") cuttingBlocks++;
    for (let j = 0; j <= steps; j++) {
      const p = pointAt(r, j / steps); samples++;
      const x0 = Math.max(0, Math.ceil((p[0] - radius - min[0]) / dx - 0.5)), x1 = Math.min(nx - 1, Math.floor((p[0] + radius - min[0]) / dx - 0.5));
      const y0 = Math.max(0, Math.ceil((p[1] - radius - min[1]) / dy - 0.5)), y1 = Math.min(ny - 1, Math.floor((p[1] + radius - min[1]) / dy - 0.5));
      for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
        fail(++visits <= 10000000, "Stock preview exceeds ten million cell checks; shorten the review or increase resolution.");
        if (Math.hypot(min[0] + (x + 0.5) * dx - p[0], min[1] + (y + 0.5) * dy - p[1]) > radius) continue;
        const index = y * nx + x, top = heights[index];
        if (top <= min[2] || p[2] >= top || p[2] + tool.cuttingLength <= min[2]) continue;
        if (r.kind === "rapid") { intersection = true; continue; }
        fail(p[2] + tool.cuttingLength >= top, `Line ${r.line} requires a buried/undercut volume or a longer cutter; a top-surface heightfield cannot represent it.`);
        heights[index] = Math.max(min[2], Math.min(top, p[2]));
      }
    }
    if (intersection) rapidIntersections.push(r.line);
  }
  fail(cuttingBlocks > 0, "No resolved cutting motions are available for this stock.");
  const initialVolume = (max[0] - min[0]) * (max[1] - min[1]) * (max[2] - min[2]);
  const removedVolume = heights.reduce((sum, top) => sum + (max[2] - top) * dx * dy, 0);
  return {schema: "linuxcnc-next-nc/stock-preview/1", status: "approximate", machine: "mill", units: review.units,
    gcodeSHA256: review.gcodeSHA256, programFingerprint: review.programFingerprint,
    setupSHA256: crypto.createHash("sha256").update(JSON.stringify(setup)).digest("hex"), workOffset: setup.workOffset,
    min, max, nx, ny, dx, dy, heights, samples, cellChecks: visits, initialVolume, removedVolume, remainingVolume: initialVolume - removedVolume,
    unresolvedLines, rapidIntersections,
    limitations: ["Approximate top-surface stock removal sampled at cell centres; small edge features can be missed. This is not a collision or clearance certificate.",
      "Only an axis-aligned block, vertical flat-end cutters and one explicit work coordinate frame are modeled. No holders, fixtures, tool deflection or lathe stock.",
      "Rapid intersections are sampled warnings and do not remove material. Machine-frame and unknown-position moves are excluded and listed.",
      "Grid and path sampling affect volume accuracy; preview geometry never changes the translated G-code."]};
}
module.exports = {stockPreview};
