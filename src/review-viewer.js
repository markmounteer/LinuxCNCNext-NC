"use strict";
const crypto = require("node:crypto");
const {reviewClient} = require("./review-client"), {pointAt} = require("./review-geometry");
const escape = v => String(v).replace(/[&<>"']/g, c => ({"&":"&amp;", "<":"&lt;", ">":"&gt;", '"':"&quot;", "'":"&#39;"}[c]));
function viewer(review, stock) {
  // HTML normalizes CRLF before CSP hashing; normalize source regardless of
  // the checkout's core.autocrlf setting so file:// Windows reviews also run.
  const script = `(${reviewClient.toString()})(${pointAt.toString()});`.replace(/\r\n?/g, "\n");
  const csp = "script-src 'sha256-" + crypto.createHash("sha256").update(script).digest("base64") + "';";
  const html = `<p>Select a line or click a segment to inspect its source, command and retained state. Green: cutting; dashed grey: rapid; orange: selected. Line playback is sequential, not real-time simulation. Coordinates in different frames never share a view.</p>
    <div class="review-controls"><label>Coordinate frame <select id="review-frame"></select></label><label>Operation <select id="review-operation"></select></label><label>Projection <select id="review-projection"><option value="iso">XYZ isometric</option><option value="XY">XY</option><option value="XZ">Z horizontal / X vertical</option><option value="YZ">YZ</option></select></label>
    <label><input id="show-rapids" type="checkbox" checked> Rapids</label><label><input id="show-cuts" type="checkbox" checked> Cuts</label><button id="view-fit">Fit view</button><button id="view-in">Zoom in</button><button id="view-out">Zoom out</button></div>
    <canvas id="path-canvas" width="900" height="500" aria-label="Selectable archived toolpath geometry">Enable JavaScript for the toolpath viewer. Metrics and line tables remain available below.</canvas><p id="view-status" role="status"></p>
    <div class="review-controls"><button id="line-prev">Previous line</button><label>G-code line <input id="review-line" type="number" min="1" value="1"></label><button id="line-next">Next line</button><button id="line-play">Play line sequence</button></div>
    <input id="review-slider" type="range" min="1" value="1" aria-label="Selected G-code line"><h3 id="line-title"></h3><pre id="line-command"></pre><select id="line-list" size="7" aria-label="Nearby G-code lines"></select><details open><summary>Selected line state and source</summary><pre id="line-detail"></pre></details>
    <noscript><p>JavaScript is disabled. Use the static metrics and source-map tables below.</p></noscript>`;
  return {html, csp, script: `<template id="review-data">${escape(JSON.stringify({review, stock}))}</template><script>${script}</script>`};
}
module.exports = {viewer};
