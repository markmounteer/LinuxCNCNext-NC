"use strict";
// Development producer only. Native preparation and execution do not use Node.
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
const manifest={schema:"nextnc-task/spindle-fixtures/1",producer:pin,generatorSHA256:hash(fs.readFileSync(__filename)),
  scope:"Synthetic tests only; not physical setup, encoder qualification or clearance approval",cases:[]};
for(const units of ["mm","inch"])for(const clockwise of [true,false]){
  const name=`lathe-${units}-${clockwise?"cw":"ccw"}-mixed`,scale=units==="inch"?25.4:1;
  const point=(x,z)=>[x/scale,0,z/scale];
  const perRev=value=>({mode:"perRevolution",value:value/scale});
  const css=speed=>({mode:"css",speed:speed/scale,maximumRPM:1800,clockwise});
  const p=new Program({machine:"lathe",units,profileRevision:2,name,timestamp:"2026-10-01T00:00:00Z"});
  const setup={schema:"linuxcnc-next-nc/execution-plan/4",machine:"lathe",units,
    tools:{"1:2":{tool:1,offset:2},"2:3":{tool:2,offset:3}},workOffsets:{"1":"G54","2":"G55"},sections:[],
    end:[{x:25/scale},{z:10/scale}]};
  for(let index=0;index<2;index++){
    const s=p.addSection({name:`spindle-${index}`,start:point(8,2),tool:{number:index+1,offset:index+2},
      workOffset:index+1,spindle:css(80000),coolant:"off",tolerance:{value:0.002/scale,provenance:"source-declared"}});
    s.setMovement("cutting");
    s.linear(point(8,0),perRev(0.18));
    s.circular(point(8,0),point(7,0),false,perRev(0.18),"XZ",2*Math.PI);
    s.rapid(point(9,0));
    s.linear(point(9,-1),perRev(0.18));
    s.linear(point(8,-1),{mode:"perMinute",value:120/scale});
    s.setSpindle({mode:"rpm",speed:600,clockwise});
    s.linear(point(8,-2),perRev(0.18));
    s.setSpindle(css(40000));
    s.linear(point(6,-2),perRev(0.3));
    s.dwell(0.01);
    setup.sections.push({mode:"retract",retract:[{x:25/scale},{z:10/scale}],
      approach:[{z:2/scale},{x:8/scale}]});
  }
  const source=p.toSTEP();setup.programFingerprint=inspect(source).report.programFingerprint.value;
  const plan=JSON.stringify(setup,null,2)+"\n";
  fs.writeFileSync(path.join(out,name+".stpnc"),source,{flag:"wx"});
  fs.writeFileSync(path.join(out,name+".plan.json"),plan,{flag:"wx"});
  manifest.cases.push({name,units,clockwise,sourceSHA256:hash(source),setupSHA256:hash(plan)});
}
fs.writeFileSync(path.join(out,"manifest.json"),JSON.stringify(manifest,null,2)+"\n",{flag:"wx"});
console.log(`${manifest.cases.length} synthetic spindle fixtures written`);
