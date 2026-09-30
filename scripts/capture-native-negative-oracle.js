"use strict";
// Test-only observation hook: preserve original return values, exceptions and
// validation order. Capture actual rejected inputs, not guessed failure cases.
// Run: node --require ./scripts/capture-native-negative-oracle.js --test test/...
const fs=require("node:fs"),path=require("node:path"),crypto=require("node:crypto"),Module=require("node:module");
const root=path.resolve(__dirname,".."),out=path.join(root,"tests-rust/fixtures/negative");
fs.mkdirSync(out,{recursive:true});
const original=Module._load,seen=new WeakSet(),source=new WeakMap();
function save(stage,text,plan,table,error){
  if(typeof text!=="string" || Buffer.byteLength(text)>1024*1024 || typeof error.code!=="string")return;
  const value={stage,text,...(plan?{plan}:{}),...(table===undefined?{}:{toolTable:table}),expected:{code:error.code,message:error.message}};
  const content=JSON.stringify(value)+"\n",id=crypto.createHash("sha256").update(content).digest("hex");
  try{fs.writeFileSync(path.join(out,id+".json"),content,{flag:"wx"});}catch(e){if(e.code!=="EEXIST")throw e;}
}
Module._load=function(request,parent,isMain){
  const result=original.apply(this,arguments);
  if(!result||typeof result!=="object"||seen.has(result))return result;
  const resolved=Module._resolveFilename(request,parent,isMain);
  if(resolved===path.join(root,"src/profile.js")){
    seen.add(result);const run=result.readProgram;
    result.readProgram=function(text){try{const p=run(text);source.set(p,text);return p;}catch(e){save("profile",text,null,undefined,e);throw e;}};
  }else if(resolved===path.join(root,"src/plan.js")){
    seen.add(result);const run=result.validatePlan;
    result.validatePlan=function(plan,program){try{return run(plan,program);}catch(e){save("plan",source.get(program),plan,undefined,e);throw e;}};
  }else if(resolved===path.join(root,"src/tool-table.js")){
    seen.add(result);const run=result.checkToolTable;
    result.checkToolTable=function(table,program,plan){try{return run(table,program,plan);}catch(e){save("tool-table",source.get(program),plan,table,e);throw e;}};
  }
  return result;
};
