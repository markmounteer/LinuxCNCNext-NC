"use strict";
// Development-only migration oracle. The native executable never invokes Node.
const fs=require("node:fs"),path=require("node:path"),crypto=require("node:crypto");
const {semanticFixture,referenceGcode}=require("../test/support/semantic-fixture");
const {readProgram}=require("../src/profile"),{translate}=require("../src/translate");
const {validateProfileShape}=require("../src/profile-shape"),{parse}=require("../vendor/fusion360next-nc/part21");
const root=path.resolve(__dirname,".."),target=path.join(root,"tests-rust","fixtures","legacy");
fs.mkdirSync(target,{recursive:true});
const hash=x=>crypto.createHash("sha256").update(x).digest("hex");
const files=fs.readdirSync(path.join(root,"src")).filter(f=>f.endsWith(".js")).map(f=>"src/"+f)
  .concat(fs.readdirSync(path.join(root,"vendor/fusion360next-nc")).filter(f=>f.endsWith(".js")).map(f=>"vendor/fusion360next-nc/"+f));
const manifest={schema:"nextnc-native/migration-baseline/1",legacyCommit:"c136b14a756cdea91d2c749a8548546c44e7cc84",sources:files.map(file=>({file,sha256:hash(fs.readFileSync(path.join(root,file)))})),cases:[]};
for (const machine of ["lathe","mill"]) for (const units of ["mm","inch"]) {
  const name=machine+"-"+units,fixture=semanticFixture(machine,units);
  fixture.text=fixture.text.replace(/\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z/g,"2026-09-30T00:00:00Z");
  const program=readProgram(fixture.text),out=translate(fixture.text,fixture.plan,{toolTable:fixture.toolTable});
  const content={...fixture,referenceGcode:referenceGcode(machine,units),program,shape:validateProfileShape(parse(fixture.text)),translation:out};
  fs.writeFileSync(path.join(target,name+".stpnc"),fixture.text);
  fs.writeFileSync(path.join(target,name+".json"),JSON.stringify(content,null,2)+"\n");
  manifest.cases.push({name,inputSHA256:hash(fixture.text),fingerprint:program.report.programFingerprint.value});
}
fs.writeFileSync(path.join(target,"manifest.json"),JSON.stringify(manifest,null,2)+"\n");
const contract=JSON.stringify(require("../src/profile-contract"),null,2)+"\n";
fs.writeFileSync(path.join(root,"src-rust/profile-shape.json"),contract);
console.log(`Captured ${manifest.cases.length} migration cases and ${files.length} source identities.`);
