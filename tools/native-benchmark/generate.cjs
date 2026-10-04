"use strict";
// Development-only synthetic sources. Nothing here is a physical setup or a CAM job.
const fs = require("node:fs"), path = require("node:path"), cp = require("node:child_process");
const {createHash} = require("node:crypto");
const PIN = "b31904b3d5ad9d56cc4cc16b8090e768b0e1958d";
const hash = x => createHash("sha256").update(x).digest("hex");
const [producerArg, outputArg, sizeArg = "1000,10000,34000"] = process.argv.slice(2);
if (!producerArg || !outputArg) throw Error("usage: node generate.cjs PINNED_FUSION_REPO NEW_OUTPUT_DIR [COUNTS]");
const producer = path.resolve(producerArg), out = path.resolve(outputArg);
const git = (...args) => cp.execFileSync("git", ["-C", producer, ...args], {encoding:"utf8"}).trim();
if (git("rev-parse", "HEAD") !== PIN || git("status", "--porcelain")) throw Error(`Producer must be clean at ${PIN}`);
const sizes = sizeArg.split(",").map(Number);
if (!sizes.length || sizes.some(n => !Number.isSafeInteger(n) || n < 4 || n > 100000 || n % 4)) throw Error("Counts must be multiples of four in [4,100000]");
const {Program} = require(path.join(producer, "src/next-nc.js"));
const {inspect} = require(path.join(producer, "lib/inspect.js"));
fs.mkdirSync(out); // Fresh directory only: never overwrite evidence.
const save = (file, data) => fs.writeFileSync(path.join(out,file), typeof data === "string" ? data : JSON.stringify(data,null,2)+"\n", {flag:"wx"});
const manifest = {schema:"nextnc-native/benchmark-inputs/1", producerCommit:PIN, generatorSHA256:hash(fs.readFileSync(__filename)),
  producerFiles:["src/next-nc.js","lib/inspect.js","lib/native-profile.js","lib/part21.js","lib/validation-error.js"].map(file=>({file,sha256:hash(fs.readFileSync(path.join(producer,file)))})),
  scope:"Deterministic synthetic compiler/resource fixtures; no physical setup or motion authorization", cases:[]};
function fixture(machine, count, units, kind) {
  const name = `${machine}-${units}-${kind}-${count}`;
  const mill = machine === "mill", radius = units === "mm" ? 2 : 0.125;
  const start = mill ? [radius,0,0] : [4*radius,0,radius];
  const p = new Program({machine,units,profileRevision:2,name,timestamp:"2026-09-30T00:00:00Z"});
  const s = p.addSection({name:"synthetic repeated geometry",start,tool:{number:1,offset:2},workOffset:1,
    spindle:mill?{mode:"rpm",speed:1000,clockwise:true}:{mode:"css",speed:units==="mm"?80000:1200,maximumRPM:1800,clockwise:true},
    coolant:"flood",tolerance:{value:units==="mm"?0.002:0.0001,provenance:"source-declared"}});
  const feed = mill ? {mode:"perMinute",value:units==="mm"?120:5} : {mode:"perRevolution",value:units==="mm"?0.18:0.007};
  let helices=0, multiTurns=0, dwells=0;
  for (let i=0;i<count;i++) {
    // Exact quarter-circle endpoints avoid trig accumulation and keep bounded extents.
    // Every fourth mill arc is a 2.25-turn helix whose rise alternates +/- 0.25.
    const a = (i+1)%4, uv = [[radius,0],[0,radius],[-radius,0],[0,-radius]][a];
    const rise = mill && i%4===3 ? ((i/4|0)%2 ? -0.25 : 0.25) : 0;
    const z = mill ? (Math.floor((i+1)/4)%2 ? 0.25 : 0) : 0;
    if (kind === "polyline") {
      const end = mill ? [radius+(i+1)*0.00001,0,0] : [4*radius,0,radius-(i+1)*0.00001];
      s.setMovement("cutting"); s.linear(end,feed);
    } else {
      const end = mill ? [uv[0],uv[1],z] : [4*radius+uv[1],0,uv[0]];
      const beforeZ = mill ? (Math.floor(i/4)%2 ? 0.25 : 0) : 0;
      const center = mill ? [0,0,beforeZ] : [4*radius,0,0];
      s.setMovement(rise ? "ramp-helix" : "cutting");
      s.circular(end,center,false,feed,mill?"XY":"XZ",rise?4.5*Math.PI:Math.PI/2);
      if (rise) { helices++; multiTurns++; }
    }
    // Preserve real ordered process boundaries at a fixed frequency in large jobs.
    if ((i+1)%1000===0) { s.dwell(0.001); s.setCoolant((i/1000|0)%2?"flood":"mist"); dwells++; }
  }
  const text=p.toSTEP(), decoded=inspect(text);
  const retract=mill?[{z:20},{x:0},{y:0}]:[{x:25},{z:10}];
  const setup={schema:"linuxcnc-next-nc/execution-plan/4",machine,units,programFingerprint:decoded.report.programFingerprint.value,
    tools:{"1:2":{tool:1,offset:2}},workOffsets:{"1":"G54"},sections:[{mode:"retract",retract,
      approach:mill?[{z:10},{x:start[0]},{y:start[1]},{z:start[2]}]:[{z:start[2]},{x:start[0]}]}],end:retract};
  const planText=JSON.stringify(setup,null,2)+"\n";
  const expected={motions:count,lines:kind==="polyline"?count:0,circular:kind==="arcs"?count:0,helices,multiTurns,dwells,
    fingerprint:decoded.report.programFingerprint.value,machine,units};
  const item={name,source:`${name}.stpnc`,setup:`${name}.plan.json`,expected,sourceBytes:Buffer.byteLength(text),setupBytes:Buffer.byteLength(planText),
    sourceSHA256:hash(text),setupSHA256:hash(planText),entities:p.lastExport.entities};
  save(item.source,text);save(item.setup,planText);save(`${name}.case.json`,item);manifest.cases.push(item);
  process.stdout.write(`${name}: ${item.sourceBytes} bytes, ${item.entities} entities\n`);
}
for (const count of sizes) for (const machine of ["mill","lathe"]) fixture(machine,count,"mm","arcs");
for (const machine of ["mill","lathe"]) { fixture(machine,sizes[0],"inch","arcs"); fixture(machine,sizes.at(-1),"mm","polyline"); }
save("manifest.json",manifest);
