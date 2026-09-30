"use strict";
// Original, dependency-free browser code. Serialized into the offline report;
// all archive values enter the DOM through textContent, never HTML or code.
function reviewClient(pointAt) {
  const data = JSON.parse(document.getElementById("review-data").content.textContent);
  const review = data.review, records = review.records, stock = data.stock;
  const $ = id => document.getElementById(id), canvas = $("path-canvas"), ctx = canvas.getContext("2d");
  const number = n => n == null ? "Unknown" : Number(n.toPrecision(7)).toString();
  const option = (parent, value, label) => { const o = document.createElement("option"); o.value = value; o.textContent = label; parent.append(o); };
  const motions = records.filter(r => r.frame && ["rapid", "linear", "arc"].includes(r.kind));
  for (const f of review.frames.slice().sort((a, b) => a.frame === "machine" ? 1 : b.frame === "machine" ? -1 : a.frame.localeCompare(b.frame))) option($("review-frame"), f.frame, f.frame);
  option($("review-operation"), "all", "All operations");
  for (const op of review.operations) option($("review-operation"), op.section, op.section + ": " + op.operation);
  $("review-projection").value = review.machine === "lathe" ? "XZ" : "iso";
  $("review-line").max = $("review-slider").max = records.length;
  let selected = motions.find(r => r.drawable)?.line || 1, rendered = [], displayTruncated = false;
  let view = {scale: 1, x: 0, y: 0}, projected = [], playback = null;
  function project(p) {
    switch ($("review-projection").value) {
      case "XY": return [p[0], -p[1]];
      case "XZ": return [p[2], -p[0]]; // lathe: Z horizontal, X radius vertical
      case "YZ": return [p[1], -p[2]];
      default: return [(p[0] - p[1]) * Math.sqrt(3) / 2, (p[0] + p[1]) / 2 - p[2]];
    }
  }
  function rebuild() {
    let count = 0; rendered = []; displayTruncated = false;
    for (const r of motions) {
      if (!r.drawable || r.frame !== $("review-frame").value || ($("review-operation").value !== "all" && r.section !== Number($("review-operation").value))) continue;
      if (r.kind === "rapid" ? !$("show-rapids").checked : !$("show-cuts").checked) continue;
      const steps = r.arc ? Math.max(8, Math.ceil(Math.abs(r.arc.sweep) * 24)) : 1;
      if (count + steps + 1 > 200000) { displayTruncated = true; continue; }
      const points = Array.from({length: steps + 1}, (_, i) => project(pointAt(r, i / steps)));
      rendered.push({r, points}); count += points.length;
    }
    fit();
  }
  function fit() {
    let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
    for (const item of rendered) for (const p of item.points) { x0 = Math.min(x0, p[0]); x1 = Math.max(x1, p[0]); y0 = Math.min(y0, p[1]); y1 = Math.max(y1, p[1]); }
    if (!rendered.length) { view = {scale: 1, x: 450, y: 250}; draw(); return; }
    const scale = Math.min(820 / Math.max(x1 - x0, 1e-9), 400 / Math.max(y1 - y0, 1e-9));
    view = {scale, x: 450 - (x0 + x1) / 2 * scale, y: 245 - (y0 + y1) / 2 * scale}; draw();
  }
  function draw() {
    ctx.clearRect(0, 0, 900, 500); ctx.fillStyle = "#f5f8fc"; ctx.fillRect(0, 0, 900, 500);
    projected = [];
    const drawItem = ({r, points}, active) => {
      const ps = points.map(p => [p[0] * view.scale + view.x, p[1] * view.scale + view.y]);
      projected.push({r, points: ps});
      ctx.beginPath(); ctx.moveTo(...ps[0]); for (const p of ps.slice(1)) ctx.lineTo(...p);
      ctx.strokeStyle = active ? "#c64d08" : r.kind === "rapid" ? "#778294" : "#127568";
      ctx.lineWidth = active ? 4 : 1.5; ctx.setLineDash(r.kind === "rapid" && !active ? [5, 4] : []); ctx.stroke(); ctx.setLineDash([]);
      if (active) { ctx.beginPath(); ctx.arc(...ps.at(-1), 5, 0, 2 * Math.PI); ctx.fillStyle = "#c64d08"; ctx.fill(); }
    };
    for (const item of rendered) if (item.r.line !== selected) drawItem(item, false);
    const active = rendered.find(item => item.r.line === selected); if (active) drawItem(active, true);
    ctx.fillStyle = "#25364b"; ctx.font = "15px system-ui";
    ctx.fillText($("review-frame").value + " · " + $("review-projection").value + " · " + review.units + (review.machine === "lathe" ? " · X radius" : ""), 15, 25);
    if (!rendered.length) ctx.fillText("No resolved segments in this view", 300, 250);
    const unknown = motions.filter(r => !r.drawable && r.frame === $("review-frame").value && ($("review-operation").value === "all" || r.section === Number($("review-operation").value))).length;
    $("view-status").textContent = `${rendered.length} segments shown. ${unknown} moves in this frame/operation have incomplete coordinates and are not drawn.` +
      (displayTruncated ? " Display capped at 200,000 vertices; metrics still cover all recorded moves." : "") +
      (!active ? " Selected line has no visible segment in this view." : " Selected segment is orange; endpoint is marked.");
  }
  function selectLine(value) {
    selected = Math.max(1, Math.min(records.length, Math.round(Number(value)) || 1));
    $("review-line").value = $("review-slider").value = selected;
    const r = records[selected - 1];
    $("line-title").textContent = `Line ${r.line} · ${r.operation} · ${r.phase || "program"}${r.path ? " · path " + r.path : ""}`;
    $("line-command").textContent = r.gcode || "G-code text not recorded in this archive; typed command shown below.";
    $("line-detail").textContent = JSON.stringify({command: r.command, frame: r.frame, start: r.start, end: r.end,
      distance: r.distance, distanceUnit: review.units, idealFeedSeconds: r.idealFeedSeconds, state: r.state, source: r.provenance}, null, 2);
    $("line-list").replaceChildren();
    for (const item of records.slice(Math.max(0, selected - 11), Math.min(records.length, selected + 10))) option($("line-list"), item.line, item.line + "  " + (item.gcode || item.kind));
    $("line-list").value = selected; draw();
  }
  for (const id of ["review-frame", "review-projection", "show-rapids", "show-cuts"]) $(id).addEventListener("change", rebuild);
  $("review-operation").addEventListener("change", () => {
    const r = motions.find(m => m.drawable && ($("review-operation").value === "all" || m.section === Number($("review-operation").value)));
    if (r) { $("review-frame").value = r.frame; selectLine(r.line); } rebuild();
  });
  for (const id of ["review-line", "review-slider", "line-list"]) $(id).addEventListener("input", e => selectLine(e.target.value));
  $("line-prev").addEventListener("click", () => selectLine(selected - 1));
  $("line-next").addEventListener("click", () => selectLine(selected + 1));
  $("view-fit").addEventListener("click", fit);
  function zoom(factor) { view.x = 450 + (view.x - 450) * factor; view.y = 250 + (view.y - 250) * factor; view.scale *= factor; draw(); }
  $("view-in").addEventListener("click", () => zoom(1.4)); $("view-out").addEventListener("click", () => zoom(1 / 1.4));
  $("line-play").addEventListener("click", () => {
    if (playback) { clearInterval(playback); playback = null; $("line-play").textContent = "Play line sequence"; return; }
    $("line-play").textContent = "Pause";
    playback = setInterval(() => { if (selected === records.length) { clearInterval(playback); playback = null; $("line-play").textContent = "Play line sequence"; } else selectLine(selected + 1); }, 180);
  });
  canvas.addEventListener("click", e => {
    const rect = canvas.getBoundingClientRect(), x = (e.clientX - rect.left) * 900 / rect.width, y = (e.clientY - rect.top) * 500 / rect.height;
    let closest = 12, line;
    for (const item of projected) for (let i = 1; i < item.points.length; i++) {
      const a = item.points[i - 1], b = item.points[i], dx = b[0] - a[0], dy = b[1] - a[1];
      const t = Math.max(0, Math.min(1, ((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy || 1)));
      const d = Math.hypot(x - a[0] - t * dx, y - a[1] - t * dy);
      if (d < closest) { closest = d; line = item.r.line; }
    }
    if (line) selectLine(line);
  });
  if (stock) {
    const sc = $("stock-canvas"), sx = sc.getContext("2d"), spanX = stock.max[0] - stock.min[0], spanY = stock.max[1] - stock.min[1];
    const scale = Math.min(720 / spanX, 360 / spanY), left = (800 - spanX * scale) / 2, top = (420 - spanY * scale) / 2;
    function drawStock() {
    sx.fillStyle = "#f5f8fc"; sx.fillRect(0, 0, 800, 440);
    for (let y = 0; y < stock.ny; y++) for (let x = 0; x < stock.nx; x++) {
      const depth = (stock.max[2] - stock.heights[y * stock.nx + x]) / (stock.max[2] - stock.min[2]);
      sx.fillStyle = `hsl(${210 - depth * 180} 65% ${85 - depth * 40}%)`;
      sx.fillRect(left + x * stock.dx * scale, top + (stock.ny - 1 - y) * stock.dy * scale, stock.dx * scale + 0.5, stock.dy * scale + 0.5);
    }
    sx.fillStyle = "#25364b"; sx.font = "14px system-ui";
    sx.fillText(`Final top surface · ${stock.workOffset} · X right / Y up · ${stock.units}`, 12, 20);
    sx.fillText(`X ${number(stock.min[0])}…${number(stock.max[0])}; Y ${number(stock.min[1])}…${number(stock.max[1])}; Z ${number(stock.min[2])}…${number(stock.max[2])}`, 12, 430);
    }
    drawStock();
    // Refresh when the below-fold surface becomes visible, including in
    // embedded browsers that discard offscreen canvas backing surfaces.
    if (typeof IntersectionObserver !== "undefined") new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) drawStock();
    }).observe(sc);
    for (const button of document.querySelectorAll("[data-review-line]")) button.addEventListener("click", () => {
      $("review-frame").value = records[Number(button.dataset.reviewLine) - 1].frame;
      $("review-operation").value = "all"; $("show-rapids").checked = true; rebuild(); selectLine(button.dataset.reviewLine); $("review-line").focus();
    });
  }
  rebuild(); selectLine(selected);
  if (typeof IntersectionObserver !== "undefined") new IntersectionObserver(entries => {
    if (entries.some(entry => entry.isIntersecting)) draw();
  }).observe(canvas);
}
module.exports = {reviewClient};
