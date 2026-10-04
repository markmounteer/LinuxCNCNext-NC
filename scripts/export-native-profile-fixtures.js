"use strict";
// Capture the independent producer/reader as portable Rust test data.
// Node and Autodesk are development tools, never native runtime dependencies.
const fs=require("node:fs"),path=require("node:path"),crypto=require("node:crypto"),cp=require("node:child_process");
const fusion=path.resolve(process.argv[2]||"../Fusion360Next-NC");
const {Program}=require(path.join(fusion,"src/next-nc")),{inspect}=require(path.join(fusion,"lib/inspect"));
const target=path.resolve("tests-rust/fixtures/native"); fs.mkdirSync(target,{recursive:true});
const hash=x=>crypto.createHash("sha256").update(x).digest("hex");
const sources=["src/next-nc.js","src/fusion-adapter.js","lib/inspect.js","lib/native-profile.js","lib/part21.js","lib/validation-error.js","posts/next-nc.cps"]
  .filter(f=>fs.existsSync(path.join(fusion,f))).map(file=>({file,sha256:hash(fs.readFileSync(path.join(fusion,file)))}));
const manifest={schema:"nextnc-native/profile-fixtures/1",producerCommit:cp.execFileSync("git",["-C",fusion,"rev-parse","HEAD"],{encoding:"utf8"}).trim(),sources,cases:[]};
function capture(name,text,origin){
  const result=inspect(text),expected={model:result.model,fingerprint:result.report.programFingerprint,inputSHA256:hash(text)};
  fs.writeFileSync(path.join(target,name+".stpnc"),text);
  fs.writeFileSync(path.join(target,name+".json"),JSON.stringify(expected)+"\n");
  manifest.cases.push({name,origin,inputSHA256:hash(text),fingerprint:expected.fingerprint.value});
}
let n=0;
for(const machine of ["lathe","mill"]) for(const units of ["mm","inch"])
  for(const plane of machine==="mill"?["XY","XZ","YZ"]:["XZ"])
    for(const clockwise of [false,true]) for(const sweep of [Math.PI/2,2*Math.PI,5*Math.PI])
      for(const rise of machine==="mill"?[0,-3,3]:[0]) {
        const [u,v,axial]={XY:[0,1,2],XZ:[2,0,1],YZ:[1,2,0]}[plane];
        const start=[0,0,0],end=[0,0,0],center=[0,0,0];start[u]=2;
        end[u]=2*Math.cos(sweep);end[v]=2*Math.sin(sweep)*(clockwise?-1:1);end[axial]=rise;
        const p=new Program({machine,units,profileRevision:2,name:`Native geometry ${++n}`,timestamp:"2026-09-30T00:00:00Z"});
        const section=p.addSection({name:"analytic",start,tool:{number:1,offset:1},workOffset:1,spindle:{mode:"rpm",speed:1000,clockwise:true},coolant:"off",tolerance:{value:0.002,provenance:"source-declared"}});
        section.setMovement(rise?"ramp-helix":"cutting");section.circular(end,center,clockwise,{value:120,mode:machine==="lathe"?"perRevolution":"perMinute"},plane,sweep);
        capture(`synthetic-${n}`,p.toSTEP(),{type:"synthetic",machine,units,plane,clockwise,sweep,rise});
      }
const evidence=path.join(fusion,"artifacts/stage1-native");
for(const file of fs.readdirSync(evidence).filter(f=>f.endsWith(".stpnc")).sort()){
  const text=fs.readFileSync(path.join(evidence,file),"utf8");if(!text)continue;
  capture("autodesk-"+path.basename(file,".stpnc").replaceAll(" ","-"),text,{type:"autodesk-post",engine:"5.413.5",evidence:file,logSHA256:hash(fs.readFileSync(path.join(evidence,file.replace(/\.stpnc$/,".log"))))});
}
fs.writeFileSync(path.join(target,"manifest.json"),JSON.stringify(manifest,null,2)+"\n");
console.log(`Captured ${manifest.cases.length} complete native profiles (${n} synthetic plus actual posting evidence).`);
