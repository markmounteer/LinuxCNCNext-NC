"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), crypto = require("node:crypto"), fs = require("node:fs"), os = require("node:os"), path = require("node:path"), {spawnSync} = require("node:child_process");
const {Program} = require("../vendor/fusion360next-nc/next-nc"), {readProgram} = require("../src/profile"), {translate} = require("../src/translate");
const {simulationPlan, example, millingExample} = require("../scripts/example"), {stockExample, archive} = require("../scripts/review-example");
const {reviewGeometry, pointAt} = require("../src/review-geometry"), {stockPreview} = require("../src/stock-preview"), {renderReport} = require("../src/report");
const close = (a,b) => assert.ok(Math.abs(a-b) < 1e-8 * Math.max(1,Math.abs(b)), `${a} != ${b}`);
const translated = (p) => { const text = p.toSTEP(); return translate(text, simulationPlan(readProgram(text))); };
const section = start => ({name:"Independent review geometry", tool:{number:1,offset:1}, workOffset:1, start, spindle:{mode:"rpm",speed:600,clockwise:true},coolant:"off"});
const feed = {mode:"perMinute",value:60};
for (const [machine, plane, u, v] of [["lathe","XZ",2,0],["mill","XY",0,1],["mill","XZ",2,0],["mill","YZ",1,2]]) {
  test(`${machine} ${plane}: independent angular wrap, major/minor senses, both full circles and unit scaling`, () => {
    for (const units of ["mm","inch"]) {
      const k = units === "mm" ? 25.4 : 1, center = [7*k, machine === "mill" ? -9*k : 0, 11*k];
      function polar(deg) { const p = center.slice(); p[u] += 3*k*Math.cos(deg*Math.PI/180); p[v] += 3*k*Math.sin(deg*Math.PI/180); return p; }
      const p = new Program({machine,units}), s = p.addSection(section(polar(170)));
      s.arc(polar(-170),center,false,feed,false,plane); s.arc(polar(170),center,false,feed,false,plane);
      s.arc(polar(-170),center,true,feed,false,plane); s.arc(polar(170),center,true,feed,false,plane);
      s.arc(polar(170),center,false,feed,true,plane); s.arc(polar(170),center,true,feed,true,plane);
      const out = translated(p), r = reviewGeometry(out.report,out.sourceMap), arcs = r.records.filter(r => r.kind === "arc");
      assert.equal(arcs.length,6);
      for (let i=0;i<6;i++) {
        const degrees = [20,340,340,20,360,360][i]; close(arcs[i].distance,3*k*degrees*Math.PI/180);
        close(arcs[i].arc.sweep,[20,340,-340,-20,360,-360][i]*Math.PI/180);
        for (let a=0;a<3;a++) close(arcs[i].arc.center[a],center[a]);
        assert.deepEqual(pointAt(arcs[i],0),arcs[i].start); assert.deepEqual(pointAt(arcs[i],1),arcs[i].end);
      }
      const mid = pointAt(arcs[0],0.5); close(mid[u],center[u]-3*k); close(mid[v],center[v]);
      const major = pointAt(arcs[2],0.5); close(major[u],center[u]+3*k); close(major[v],center[v]);
      close(r.totals.cuttingDistance,24*Math.PI*k); assert.deepEqual(out.report.motionSummary.totals,r.totals);
      assert.equal(out.sourceMap.map(e=>e.gcode).join("\n")+"\n",out.gcode);
    }
  });
}
test("metrics retain zero moves, partial-axis distances and unknown starts; RPM and CSS time differ", () => {
  for (const machine of ["lathe","mill"]) {
    const p = new Program({machine,units:"mm"}), s = p.addSection(section([0,0,0]));
    s.linear([3,0,4],feed); s.paths[0].points.push([3,0,4]); // Preserve an explicit duplicate source vertex.
    s.linear([6,0,8],{mode:"perRevolution",value:0.1}); s.dwell(0.25); s.rapid([9,0,12]);
    const out=translated(p),r=reviewGeometry(out.report,out.sourceMap),body=r.records.filter(e=>e.phase==="toolpath"&&e.distance!==null);
    assert.deepEqual(body.map(r=>r.distance),[5,0,5,5]);
    close(r.totals.cuttingDistance,10); close(r.totals.idealFeedSeconds,10); close(r.totals.dwellSeconds,0.25);
    assert.ok(r.totals.unknownDistanceBlocks>0); assert.equal(r.totals.unknownFeedTimeBlocks,0);
    const first = r.records.find(r=>r.kind==="rapid"); assert.equal(first.distance,null); assert.equal(first.drawable,false);
    const knownPartial = r.records.find(r=>r.phase==="transition"&&r.kind==="rapid"&&r.distance!==null);
    if (machine==="mill") { assert.ok(knownPartial); assert.equal(knownPartial.distance,10); }
    assert.ok(r.frames.some(f=>f.frame==="machine")); assert.ok(r.frames.some(f=>f.frame==="work:G54"));
  }
  const f=example(),out=translate(f.text,f.plan),r=reviewGeometry(out.report,out.sourceMap);
  assert.equal(r.totals.unknownFeedTimeBlocks,3); close(r.totals.idealFeedSeconds,4*Math.PI/100*60);
  assert.ok(r.records.filter(r=>r.state.spindleMode==="css"&&r.kind!=="rapid"&&r.distance!==null).every(r=>r.idealFeedSeconds===null));
});
test("tool changes and WCS switches never join geometry across coordinate systems",()=>{
  const f=millingExample(),out=translate(f.text,f.plan),r=reviewGeometry(out.report,out.sourceMap);
  assert.deepEqual(new Set(r.frames.map(f=>f.frame)),new Set(["machine","work:G54","work:G55"]));
  for(const section of [1,2]) {
    const first=r.records.find(r=>r.section===section&&r.kind==="rapid"&&r.frame.startsWith("work:"));
    assert.deepEqual(first.start,[null,null,null]); assert.equal(first.drawable,false); assert.equal(first.distance,null);
  }
  for (const change of r.records.filter(r=>r.kind==="toolChange")) { assert.equal(change.state.spindleDirection,null); assert.equal(change.state.tool,change.command.tool); }
  assert.throws(()=>reviewGeometry(out.report,[null]),e=>e.code==="REVIEW_GEOMETRY");
  const partial=structuredClone(out.sourceMap); partial.splice(2,1); assert.throws(()=>reviewGeometry(out.report,partial),e=>e.code==="REVIEW_GEOMETRY");
});
test("stock preview removes the independently counted cells of a flat-ended plunge",()=>{
  for(const units of ["mm","inch"]) {
    const k=units==="mm"?1:1/25.4,p=new Program({machine:"mill",units}),s=p.addSection(section([0,0,2*k]));
    s.linear([0,0,-k],feed); const out=translated(p),r=reviewGeometry(out.report,out.sourceMap);
    const setup={schema:"linuxcnc-next-nc/stock-setup/1",machine:"mill",units,programFingerprint:r.programFingerprint,gcodeSHA256:r.gcodeSHA256,workOffset:"G54",stock:{min:[-2*k,-2*k,-2*k],max:[2*k,2*k,0]},resolution:0.5*k,tools:{1:{shape:"flat-end",diameter:2*k,cuttingLength:5*k}}};
    const result=stockPreview(r,setup); assert.equal(result.heights.filter(h=>h===-k).length,12);
    close(result.removedVolume,3*k*k*k); close(result.initialVolume,32*k*k*k); assert.ok(result.unresolvedLines.length>0);
    assert.equal(result.rapidIntersections.length,0); assert.equal(result.heights.length,64);
  }
});
test("stock preview flags rapid contact without removing material and keeps sampled arcs",()=>{
  const f=stockExample(),r=reviewGeometry(f.out.report,f.out.sourceMap),before=JSON.stringify(r);
  const original=stockPreview(r,f.setup);
  let expected=0;
  for(let y=0;y<48;y++) for(let x=0;x<80;x++) {
    const px=-5+(x+0.5)/8,py=-3+(y+0.5)/8,rad=Math.hypot(px,py);
    if (Math.hypot(px-Math.max(-4,Math.min(4,px)),py)<=1 || (rad>=1 && rad<=3)) expected+=1/64;
  }
  // Independent exact cell membership in the capsule/annulus union. Path
  // sampling can omit only a narrow boundary fringe at this grid resolution.
  assert.ok(original.removedVolume<=expected); assert.ok(expected-original.removedVolume<0.5);
  assert.equal(JSON.stringify(r),before); assert.equal(original.rapidIntersections.length,0);
  const rapid=structuredClone(r.records.find(r=>r.kind==="linear"));
  rapid.kind="rapid"; rapid.line=r.records.length+1; rapid.start=[-4,2,-1]; rapid.end=[4,2,-1]; rapid.distance=8;
  const altered=structuredClone(r); altered.records.push(rapid);
  const result=stockPreview(altered,f.setup); assert.deepEqual(result.heights,original.heights); assert.deepEqual(result.rapidIntersections,[rapid.line]);
});
test("stock input rejects wrong identity, missing shapes, WCS ambiguity and excessive work",()=>{
  const f=stockExample(),r=reviewGeometry(f.out.report,f.out.sourceMap);
  for(const mutate of [s=>s.gcodeSHA256="0".repeat(64),s=>s.programFingerprint=undefined,s=>s.units="inch",s=>s.machine="lathe",s=>s.workOffset="G55",s=>s.stock.max[0]=s.stock.min[0],s=>s.tools={},s=>s.tools[1].shape="ball-end",s=>s.tools[1].cuttingLength=0.1,s=>s.resolution=0.0001,s=>s.tools[1].diameter=0.1,s=>s.extra=true]) {
    const setup=structuredClone(f.setup); mutate(setup); assert.throws(()=>stockPreview(r,setup),e=>e.code==="STOCK_PREVIEW");
  }
  const missing=structuredClone(f.setup); missing.tools={2:missing.tools[1]}; assert.throws(()=>stockPreview(r,missing),/No preview geometry/);
  const f2=millingExample(),out=translate(f2.text,f2.plan),r2=reviewGeometry(out.report,out.sourceMap),s2={...f.setup,programFingerprint:r2.programFingerprint,gcodeSHA256:r2.gcodeSHA256};
  assert.throws(()=>stockPreview(r2,s2),/Multiple or unmatched work offsets/);
  const many=structuredClone(r); many.records.find(r=>r.kind==="linear").distance=1e9; assert.throws(()=>stockPreview(many,f.setup),/200000 path samples/);
  const costly=structuredClone(f.setup); costly.stock={min:[-20,-20,-3],max:[20,20,0]}; costly.tools[1].diameter=100;
  assert.throws(()=>stockPreview(r,costly),/ten million cell checks/);
});
test("HTML binds one trusted script by CSP hash and treats hostile archive values as data",()=>{
  const f=stockExample(),record=archive(f.out),hostile='</template><script>globalThis.pwned=true</script><img src="https://bad.invalid">';
  record.sourceMap[0].operation=hostile; record.input=hostile;
  const before=JSON.stringify(record),html=renderReport(record,{stockSetup:f.setup});
  assert.equal(JSON.stringify(record),before); assert.equal((html.match(/<script>/g)||[]).length,1);
  const script=html.match(/<script>([\s\S]*?)<\/script>/)[1];
  assert.ok(!script.includes("\r")); new (require("node:vm").Script)(script);
  assert.ok(html.includes("script-src 'sha256-"+crypto.createHash("sha256").update(script).digest("base64")+"'"));
  assert.doesNotMatch(script,/pwned|innerHTML|eval\(|fetch\(/); assert.doesNotMatch(html,/<img|<iframe|script src=/i);
  assert.match(html,/&lt;\/template&gt;&lt;script&gt;/); assert.match(html,/stock-canvas/); assert.match(html,/No total cycle-time claim/i);
  const legacy=archive(f.out); delete legacy.sourceMap; assert.doesNotMatch(renderReport(legacy),/<script>/);
});
test("CLI stock review is report-only, exclusive and leaves archive, stock and G-code unchanged",()=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),"nextnc-stock-"));
  try {
    const f=stockExample(),files={archive:path.join(dir,"job.json"),stock:path.join(dir,"stock.json"),gcode:path.join(dir,"job.ngc"),html:path.join(dir,"review.html")};
    const values={archive:JSON.stringify(archive(f.out)),stock:JSON.stringify(f.setup),gcode:f.out.gcode};
    for(const k of Object.keys(values)) fs.writeFileSync(files[k],values[k]);
    const run=(...args)=>spawnSync(process.execPath,[path.join(__dirname,"../bin/nextnc.js"),...args],{encoding:"utf8",env:{...process.env,NEXTNC_DIAGNOSTICS:path.join(dir,"diagnostics")}});
    const args=["report",files.archive,"--gcode",files.gcode,"--stock-setup",files.stock,"--output",files.html];
    const ok=run(...args); assert.equal(ok.status,0,ok.stderr); assert.match(fs.readFileSync(files.html,"utf8"),/Saved G-code identity: Match/);
    assert.equal(run(...args).status,1); for(const k of Object.keys(values)) assert.equal(fs.readFileSync(files[k],"utf8"),values[k]);
    assert.equal(fs.existsSync(path.join(dir,"diagnostics")),false);
    for(const cmd of ["translate","preflight","inspect","filter","doctor"]) assert.equal(run(cmd,files.archive,"--stock-setup",files.stock).status,2);
    fs.writeFileSync(files.stock,'{"machine":"lathe"}'); const target=path.join(dir,"bad.html");
    const bad=run("report",files.archive,"--stock-setup",files.stock,"--output",target); assert.equal(bad.status,1); assert.equal(fs.existsSync(target),false);
  } finally { fs.rmSync(dir,{recursive:true,force:true}); }
});
