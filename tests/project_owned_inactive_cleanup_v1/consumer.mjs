import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import instantiate from './package/semaprax.bindings.js';

const wasm=Uint8Array.from(readFileSync(new URL('./package/app.wasm',import.meta.url)));
const originalInstantiate=WebAssembly.instantiate;
const originalSet=Map.prototype.set,originalDelete=Map.prototype.delete,originalGet=Map.prototype.get;
const originalSize=Object.getOwnPropertyDescriptor(Map.prototype,'size');
const originalRead=DataView.prototype.getBigInt64;
const apply=Reflect.apply,RESULT=65536;
let current=null,memory;
const seen=new Set();
WebAssembly.instantiate=async(bytes,imports)=>{
  assert.deepEqual(new Uint8Array(bytes),wasm);
  const env={...imports.env};
  env.spx_bytes_copy=(...args)=>{
    assert(current.engine);current.minting=true;
    let carrier;
    try{carrier=imports.env.spx_bytes_copy(...args)}finally{current.minting=false}
    current.mints++;current.carrier=carrier;
    const token=Number((BigInt.asUintN(64,carrier)>>32n)&0x7fffffffn);
    assert.equal(current.sets,1);assert.equal(current.token,token);
    assert(current.owner instanceof Uint8Array);assert.deepEqual(current.owner,current.expected);
    assert(!seen.has(token),'issued token was reused');seen.add(token);
    return carrier;
  };
  env.spx_bytes_drop=(...args)=>{
    assert(current.engine);assert.equal(args.length,1);assert.equal(args[0],current.carrier);
    current.dropping=true;
    try{const result=imports.env.spx_bytes_drop(...args);current.drops++;return result}
    finally{current.dropping=false}
  };
  const result=await originalInstantiate(bytes,{...imports,env});
  const exports={...result.instance.exports};memory=exports.memory;
  let wrapped=0;
  for(const [name,fn] of Object.entries(exports))if(name.startsWith('spx_owned_v1_')){
    wrapped++;
    exports[name]=(...args)=>{
      current.entries++;current.engine=true;
      try{const status=fn(...args);assert.equal(status,0);current.returns++;return status}
      finally{current.engine=false}
    };
  }
  assert.equal(wrapped,2);
  return {...result,instance:{exports}};
};
let api;
try{api=await instantiate(wasm)}finally{WebAssembly.instantiate=originalInstantiate}

function invoke(id,input,active){
  const before=Uint8Array.from(input);
  current={engine:false,minting:false,dropping:false,map:null,token:null,carrier:null,owner:null,expected:before,
    entries:0,returns:0,sets:0,mints:0,drops:0,dropDeletes:0,consumes:0,reads:0,settles:0};
  Map.prototype.set=function(key,value){
    const result=apply(originalSet,this,[key,value]);
    if(current.minting){
      // Only record here: assertion-library Map use must not recursively
      // become another observed arena insertion. Calibration follows the
      // real import with minting already false.
      current.sets++;
      current.map=this;current.token=key;current.owner=value;
    }
    return result;
  };
  Map.prototype.delete=function(key){
    if(this===current.map&&key===current.token)assert.equal(apply(originalGet,this,[key]),current.owner);
    const result=apply(originalDelete,this,[key]);
    if(this===current.map&&key===current.token){
      assert.equal(result,true,'same owner must be deleted exactly once');
      if(current.engine){assert(current.dropping);current.dropDeletes++}
      else{assert(!current.dropping);current.consumes++}
    }
    return result;
  };
  Object.defineProperty(Map.prototype,'size',{...originalSize,get(){
    const size=apply(originalSize.get,this,[]);
    if(this===current.map&&!current.engine){assert.equal(size,0);current.settles++}
    return size;
  }});
  DataView.prototype.getBigInt64=function(...args){
    const value=apply(originalRead,this,args);
    if(!current.engine&&this.buffer===memory.buffer&&args[0]===RESULT+8)current.reads++;
    return value;
  };
  let value;
  try{value=api.call(id,input,active)}finally{
    Map.prototype.set=originalSet;Map.prototype.delete=originalDelete;
    Object.defineProperty(Map.prototype,'size',originalSize);
    DataView.prototype.getBigInt64=originalRead;
  }
  assert.equal(current.entries,1);assert.equal(current.returns,1);
  assert.equal(current.sets,1);assert.equal(current.mints,1);
  assert.equal(current.drops,active?0:1);
  assert.equal(current.dropDeletes,active?0:1);
  assert.equal(current.consumes,active?1:0);
  assert.equal(current.settles,1);
  assert.equal(apply(originalSize.get,current.map,[]),0,'the authenticated arena is empty after publication');
  assert.equal(current.reads,id==='inactive.maybe'&&!active?0:1);
  assert.deepEqual(input,before);
  assert(new Uint8Array(memory.buffer,0,input.length).every(byte=>byte===0));
  assert(new Uint8Array(memory.buffer,RESULT,16).every(byte=>byte===0xa5));
  if(id==='inactive.result'){
    assert(Object.isFrozen(value));
    if(active){assert.deepEqual(Object.keys(value),['ok','value']);assert.equal(value.ok,true);value=value.value}
    else assert.deepEqual(value,{ok:false,error:-7n});
  }else if(!active)assert.equal(value,null);
  if(active){
    assert.equal(Object.getPrototypeOf(value),Uint8Array.prototype);
    assert.deepEqual(value,before);assert.notEqual(value.buffer,input.buffer);
    assert.notEqual(value.buffer,current.owner.buffer,'host output must not adopt arena storage');
  }
  return value;
}

const corpus=[new Uint8Array(),Uint8Array.of(0,255,195,40,128),
  Uint8Array.from({length:65535},(_,i)=>i%251),
  Uint8Array.from({length:65536},(_,i)=>i%251)];
const retained=[];
for(let round=0;round<4;round++)for(const input of corpus)for(const id of ['inactive.maybe','inactive.result']){
  retained.push([invoke(id,input,true),Uint8Array.from(input)]);
  invoke(id,input,false);
  // Same object must remain usable after each inactive result and real drop.
  const recovery=Uint8Array.of(255,0,128);
  assert.deepEqual(invoke(id,recovery,true),recovery);
}
for(const [value,expected] of retained)assert.deepEqual(value,expected);
console.log('project-owned-inactive-cleanup-ok');
