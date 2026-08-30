import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {instantiateCore} from "./runtime.mjs";

const wasm=new Uint8Array(readFileSync(new URL("./app.wasm",import.meta.url)));
const descriptor=JSON.parse(readFileSync(new URL("./descriptor.json",import.meta.url),"utf8"));
const corpus=JSON.parse(readFileSync(new URL("./corpus.json",import.meta.url),"utf8"));
const ids=["frame.payload","frame.payload-maybe","frame.payload-result"];
assert.deepEqual(descriptor.exports.map(row=>row.stable_id),ids);
assert.equal(corpus.schema,"semaprax.frame-payload-corpus.v1");
assert.equal(corpus.cases.length,9);
const rawName=id=>"spx_owned_v1_"+Buffer.from(id,"utf8").toString("hex");
const module=await WebAssembly.compile(wasm);
const imported=["spx_add","spx_sub","spx_mul","spx_div","spx_rem","spx_neg","spx_contract_fail",
  "spx_bytes_copy","spx_bytes_get","spx_bytes_drop","spx_bytes_as_slice","spx_owned_utf8_validate_v1"];
assert.deepEqual(WebAssembly.Module.imports(module),imported.map(name=>({module:"env",name,kind:"function"})));
assert.deepEqual(WebAssembly.Module.exports(module),[
  {name:"memory",kind:"memory"},
  ...["__spx_data_status_v1","__spx_data_scratch_base_v1","__spx_data_scratch_capacity_v1"].map(name=>({name,kind:"global"})),
  ...ids.map(id=>({name:rawName(id),kind:"function"})),
]);

// Observe actual imported operations without replacing their meaning. The
// real production factory still authenticates bytes and owns the real arena.
let imports=0,mints=0,drops=0,consumes=0,settlements=0;
const live=new Set();
const instantiate=WebAssembly.instantiate;
let linked;
try{
  WebAssembly.instantiate=async(bytes,provided)=>{
    const env=Object.create(null);
    for(const [name,operation] of Object.entries(provided.env)){
      env[name]=(...args)=>{
        imports++;
        const result=operation(...args);
        if(name==="spx_bytes_copy"){
          assert.notEqual(result,0n);assert.equal(live.has(result),false);
          live.add(result);mints++;
        }else if(name==="spx_bytes_drop"){
          assert.equal(live.delete(args[0]),true);drops++;
        }
        return result;
      };
    }
    return instantiate.call(WebAssembly,bytes,{env});
  };
  linked=await instantiateCore(wasm);
}finally{WebAssembly.instantiate=instantiate;}
const e=linked.instance.exports;
assert.equal(e.memory.buffer.byteLength,131072);
assert.equal(e.__spx_data_scratch_base_v1.value,0);
assert.equal(e.__spx_data_scratch_capacity_v1.value,65536);
const bytes=new Uint8Array(e.memory.buffer),view=new DataView(e.memory.buffer);
const RESULT=65536,POISON=0xa5;
function settle(){linked.arena.settle();assert.equal(live.size,0);settlements++;}
function consume(carrier,expected){
  assert.equal(live.has(carrier),true);
  const output=linked.arena.consume(carrier);consumes++;
  assert.equal(live.delete(carrier),true);
  assert.equal(Object.getPrototypeOf(output),Uint8Array.prototype);
  assert.deepEqual(output,expected);
}
function materialize(row){
  if(row.kind==="hex")return {
    frame:new Uint8Array(Buffer.from(row.frame_hex,"hex")),
    payload:row.valid?new Uint8Array(Buffer.from(row.payload_hex,"hex")):null,
  };
  assert.equal(row.kind,"generated-index-mod-256");
  const payload=Uint8Array.from({length:row.payload_length},(_,index)=>index&255);
  const frame=new Uint8Array(payload.length+8);frame.set([83,80,88,49]);
  new DataView(frame.buffer).setUint32(4,payload.length,false);frame.set(payload,8);
  return {frame,payload};
}

// Alignment rejection must precede all imports, preserve the whole poisoned
// result region, and leave this same arena reusable. No semantic failure is
// fabricated by replacing an export or substituting an import result.
linked.arena.begin();bytes.fill(POISON,RESULT,RESULT+24);
const beforeImports=imports;
assert.equal(e[rawName(ids[0])](0,0,RESULT+1),11);
assert.equal(imports,beforeImports);
assert(bytes.slice(RESULT,RESULT+24).every(byte=>byte===POISON));
settle();

let calls=0,ownedCalls=0;
for(const row of corpus.cases){
  const {frame,payload}=materialize(row);
  for(const id of ids){
    if(id==="frame.payload"&&!row.valid)continue;
    linked.arena.begin();assert.equal(live.size,0);
    bytes.set(frame,0);bytes.fill(POISON,RESULT,RESULT+16);
    const priorMints=mints,priorDrops=drops,priorConsumes=consumes;
    assert.equal(e[rawName(id)](0,frame.length,RESULT),0,`${row.name}/${id}/status`);
    if(id==="frame.payload")consume(view.getBigInt64(RESULT,true),payload);
    else{
      const tag=view.getUint32(RESULT,true);
      const expectedTag=id==="frame.payload-maybe"?Number(row.valid):Number(!row.valid);
      assert.equal(tag,expectedTag,`${row.name}/${id}/tag`);
      if(row.valid)consume(view.getBigInt64(RESULT+8,true),payload);
      else if(id==="frame.payload-result")assert.equal(view.getBigInt64(RESULT+8,true),BigInt(row.error));
      // None has no active payload: deliberately never read its inactive word.
    }
    assert.equal(mints-priorMints,Number(row.valid),`${row.name}/${id}/mint`);
    assert.equal(drops-priorDrops,0,`${row.name}/${id}/internal-drop`);
    assert.equal(consumes-priorConsumes,Number(row.valid),`${row.name}/${id}/copy-out-settle`);
    settle();bytes.fill(0,0,frame.length);bytes.fill(POISON,RESULT,RESULT+16);
    calls++;if(row.valid)ownedCalls++;
  }
}
assert.equal(calls,23);assert.equal(ownedCalls,15);
assert.equal(mints,15);assert.equal(consumes,15);assert.equal(drops,0);
assert.equal(settlements,calls+1);
console.log("frame-payload-raw-wasm-v1-ok");
