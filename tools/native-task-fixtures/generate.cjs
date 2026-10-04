"use strict";
// Development fixture producer only; the executing native path is Rust.
const fs=require("node:fs"),path=require("node:path"),cp=require("node:child_process");
const {createHash}=require("node:crypto");
const [producerArg,outArg]=process.argv.slice(2);
if(!producerArg||!outArg)throw Error("usage: node generate.cjs PINNED_FUSION_REPO NEW_OUTPUT_DIR");
const producer=path.resolve(producerArg),out=path.resolve(outArg);
const pin="b31904b3d5ad9d56cc4cc16b8090e768b0e1958d";
const git=(...args)=>cp.execFileSync("git",["-C",producer,...args],{encoding:"utf8"}).trim();
if(git("rev-parse","HEAD")!==pin||git("status","--porcelain"))throw Error("Producer must be clean at the pin");
const {Program}=require(path.join(producer,"src/next-nc.js"));
const {inspect}=require(path.join(producer,"lib/inspect.js"));
const hash=data=>createHash("sha256").update(data).digest("hex");
fs.mkdirSync(out);
const manifest={schema:"nextnc-task/fixtures/1",producer:pin,generatorSHA256:hash(fs.readFileSync(__filename)),
  scope:"Synthetic simulator/unit fixtures; not physical setup or clearance approval",cases:[]};
for(const machine of ["mill","lathe"])for(const units of ["mm","inch"]){
  const shapes=machine==="mill"?["line","pure-z","arc-xy","arc-xz","arc-yz","full-xy","cw-xy","helix-xy","multi-xy"]
    :["line","arc-xz","full-xz","cw-xz"];
  for(const shape of shapes){
    const name=`${machine}-${units}-${shape}`,mill=machine==="mill";
    const plane=shape.endsWith("xz")?"XZ":shape.endsWith("yz")?"YZ":"XY";
    const [u,v,n]=({XY:[0,1,2],XZ:[2,0,1],YZ:[1,2,0]})[plane];
    const center=mill?[3,4,5]:[8,0,0];
    let start=mill?[5,4,5]:[8,0,2],end;
    const linear=shape==="line"||shape==="pure-z",clockwise=shape.startsWith("cw");
    const sweep=shape.startsWith("full")?2*Math.PI:shape.startsWith("multi")?4.5*Math.PI:Math.PI/2;
    const rise=shape.startsWith("helix")||shape.startsWith("multi")?0.5:0;
    if(linear){end=start.slice();end[2]-=1;if(shape!=="pure-z")end[0]+=1;}
    else {start=center.slice();start[u]+=2;end=center.slice();
      if(shape.startsWith("full"))end[u]+=2;else end[v]+=clockwise?-2:2;end[n]+=rise;}
    const p=new Program({machine,units,profileRevision:2,name,timestamp:"2026-09-30T00:00:00Z"});
    const section=p.addSection({name:shape,start,tool:{number:1,offset:2},workOffset:1,
      spindle:{mode:"rpm",speed:1000,clockwise:true},coolant:"flood",
      tolerance:{value:units==="mm"?0.002:0.0001,provenance:"source-declared"}});
    const feed={mode:"perMinute",value:units==="mm"?120:5};
    section.setMovement(rise?"ramp-helix":"cutting");
    if(linear)section.linear(end,feed);else section.circular(end,center,clockwise,feed,plane,sweep);
    section.dwell(0.01);
    const source=p.toSTEP(),decoded=inspect(source);
    const retract=mill?[{z:20},{x:0},{y:0}]:[{x:25},{z:10}];
    const setup=JSON.stringify({schema:"linuxcnc-next-nc/execution-plan/4",machine,units,
      programFingerprint:decoded.report.programFingerprint.value,tools:{"1:2":{tool:1,offset:2}},workOffsets:{"1":"G54"},
      sections:[{mode:"retract",retract,approach:mill?[{z:10},{x:start[0]},{y:start[1]},{z:start[2]}]:[{z:start[2]},{x:start[0]}]}],end:retract},null,2)+"\n";
    fs.writeFileSync(path.join(out,name+".stpnc"),source,{flag:"wx"});
    fs.writeFileSync(path.join(out,name+".plan.json"),setup,{flag:"wx"});
    manifest.cases.push({name,machine,units,shape,start,end,center:linear?null:center,plane:linear?null:plane,
      clockwise,sweepRadians:linear?null:sweep,axialRise:rise,sourceSHA256:hash(source),setupSHA256:hash(setup)});
  }
}
fs.writeFileSync(path.join(out,"manifest.json"),JSON.stringify(manifest,null,2)+"\n",{flag:"wx"});
console.log(`${manifest.cases.length} synthetic native task fixtures written`);
