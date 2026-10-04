"use strict";
// Development oracle only. Neither the native build nor executable invokes Node.
const fs=require("node:fs"),path=require("node:path"),cp=require("node:child_process");
const fusion=path.resolve(process.argv[2]||"../Fusion360Next-NC");
const evidence=path.resolve(process.argv[3]||path.join(fusion,"artifacts/stage1-native"));
const {inspect}=require(path.join(fusion,"lib/inspect"));
const {readProgram}=require("../src/profile");
const exe=path.resolve("target/debug/nextnc-native"+(process.platform==="win32"?".exe":""));
let count=0,failed=0;
function differences(a,b,p,out){
  if(JSON.stringify(a)===JSON.stringify(b))return;
  if(a&&b&&typeof a==="object"&&typeof b==="object"){
    if(Object.keys(a).join()!==Object.keys(b).join())out.push(`${p}: different key order`);
    for(const k of new Set([...Object.keys(a),...Object.keys(b)]))differences(a[k],b[k],p+"/"+k,out);
  }else out.push(`${p}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}
for(const name of fs.readdirSync(evidence).filter(x=>x.endsWith(".stpnc")).sort()){
  const file=path.join(evidence,name),text=fs.readFileSync(file,"utf8");
  if(!text)continue; // Failed-post logs are qualified separately by the producer.
  const result=cp.spawnSync(exe,["inspect",file],{encoding:"utf8",maxBuffer:32*1024*1024});
  count++;
  if(result.status!==0){failed++;console.error(name,result.stderr);continue;}
  const native=JSON.parse(result.stdout).program,expected=inspect(text),diff=[];
  let oldRejected=false;
  try{readProgram(text);}catch(error){if(typeof error.code!=="string")throw error;oldRejected=true;}
  if(!oldRejected)diff.push("Revision-1 consumer accepted revision-2 input");
  differences(native.model,expected.model,"model",diff);
  if(native.report.programFingerprint.value!==expected.report.programFingerprint.value)diff.push("Semantic fingerprint differs");
  if(diff.length){failed++;console.error(name,diff.join("\n"));}else console.log(`${name}: exact model and fingerprint match`);
}
if(count===0)throw new Error("No posted evidence found");
console.log(`${count-failed}/${count} actual posted programs match and revision-1 consumer rejects revision-2 input.`);
process.exitCode=failed?1:0;
