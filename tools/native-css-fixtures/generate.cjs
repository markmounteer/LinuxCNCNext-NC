"use strict";
// Development fixture producer only; no Node dependency in native execution.
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
const manifest={schema:"nextnc-task/css-fixtures/1",producer:pin,generatorSHA256:hash(fs.readFileSync(__filename)),
  scope:"Synthetic centerline/offset/CSS tests only; no real setup, clearance or encoder qualification",cases:[]};
for(const units of ["mm","inch"])for(const clockwise of [true,false]){
  const name=`lathe-${units}-${clockwise?"cw":"ccw"}-css`,scale=units==="inch"?25.4:1;
  const point=(x,z)=>[x/scale,0,z/scale],feed={mode:"perRevolution",value:0.18/scale};
  const css=(surface,cap)=>({mode:"css",speed:surface/scale,maximumRPM:cap,clockwise});
  const p=new Program({machine:"lathe",units,profileRevision:2,name,timestamp:"2026-10-02T00:00:00Z"});
  const setup={schema:"linuxcnc-next-nc/execution-plan/4",machine:"lathe",units,
    tools:{"1:2":{tool:1,offset:2},"1:3":{tool:1,offset:3},"2:3":{tool:2,offset:3}},
    workOffsets:{"1":"G54","2":"G55"},sections:[],end:[{x:25/scale},{z:10/scale}]};
  const sections=[{tool:1,offset:2,work:1,surface:8000,cap:1800},
    {tool:1,offset:3,work:1,surface:6000,cap:1200},
    {tool:2,offset:3,work:2,surface:10000,cap:1600}];
  for(const [index,c] of sections.entries()){
    const s=p.addSection({name:`css-offset-${index}`,start:point(4,1),tool:{number:c.tool,offset:c.offset},
      workOffset:c.work,spindle:css(c.surface,c.cap),coolant:"off",
      tolerance:{value:0.002/scale,provenance:"source-declared"}});
    s.setMovement("cutting");
    s.linear(point(4,0),feed);
    s.linear(point(0,0),feed);
    s.dwell(0.25);
    s.setSpindle(css(c.surface,c.cap/2));
    s.dwell(0.25);
    s.linear(point(0,-1),feed);
    s.linear(point(-2,-1),feed);
    s.circular(point(-2,-1),point(-3,-1),false,feed,"XZ",2*Math.PI);
    setup.sections.push({mode:"retract",retract:[{x:25/scale},{z:10/scale}],
      approach:[{z:1/scale},{x:4/scale}]});
  }
  const source=p.toSTEP();setup.programFingerprint=inspect(source).report.programFingerprint.value;
  const plan=JSON.stringify(setup,null,2)+"\n";
  fs.writeFileSync(path.join(out,name+".stpnc"),source,{flag:"wx"});
  fs.writeFileSync(path.join(out,name+".plan.json"),plan,{flag:"wx"});
  manifest.cases.push({name,units,clockwise,sections,sourceSHA256:hash(source),setupSHA256:hash(plan)});
}
fs.writeFileSync(path.join(out,"manifest.json"),JSON.stringify(manifest,null,2)+"\n",{flag:"wx"});
console.log(`${manifest.cases.length} synthetic CSS fixtures written`);
