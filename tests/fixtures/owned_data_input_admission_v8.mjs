import fs from 'node:fs';
import assert from 'node:assert/strict';

// Optional profile facts come from the test's authenticated descriptor, not
// from a replacement runtime. The original v8 invocation supplies none.
const profile=process.argv[2]===undefined?{}:JSON.parse(process.argv[2]);
// Observe production snapshot operations; do not substitute an arena/runtime.
const OriginalUint8=Uint8Array;
const typedPrototype=Object.getPrototypeOf(Uint8Array.prototype);
const originalSet=typedPrototype.set,originalEncode=TextEncoder.prototype.encode;
let allocations=0,copies=0,encodings=0;
globalThis.Uint8Array=new Proxy(OriginalUint8,{
  construct(target,args){allocations++;return Reflect.construct(target,args,target)}
});
typedPrototype.set=function(...args){copies++;return Reflect.apply(originalSet,this,args)};
TextEncoder.prototype.encode=function(...args){encodings++;return Reflect.apply(originalEncode,this,args)};
const {default:instantiate}=await import('./semaprax.bindings.js');
const moduleBytes=new OriginalUint8(fs.readFileSync(new URL('./app.wasm',import.meta.url)));
const api=await instantiate(moduleBytes),call=api.functions['probe.bytes'];
const reset=()=>{allocations=0;copies=0;encodings=0};
const empty=new OriginalUint8();
function rejected(args,kind=RangeError,invoke=call){
  reset();assert.throws(()=>invoke(...args),kind);
  assert.deepEqual([allocations,copies,encodings],[0,0,0],'reject before any payload snapshot');
}
function bytes(value){
  if(value instanceof OriginalUint8)return value;
  if(profile.recordFields){
    const fields=profile.recordFields;
    assert.equal(Object.getPrototypeOf(value),null);assert.equal(Object.isFrozen(value),true);
    assert.deepEqual(Object.keys(value),[fields.kind,fields.valid,fields.size,fields.payload]);
    assert.equal(value[fields.kind],7n);assert.equal(typeof value[fields.valid],'boolean');
    const payload=value[fields.payload];assert.equal(Object.getPrototypeOf(payload),OriginalUint8.prototype);
    assert.equal(value[fields.size],BigInt(payload.length));return payload;
  }
  assert.equal(value.ok,true);return value.value;
}
for(const size of [0,65535,65536]){
  const input=new OriginalUint8(size);if(size)input[size-1]=255;
  const output=bytes(call(input,'',empty));
  assert.notEqual(output,input);assert.deepEqual(output,input);
}
rejected([new OriginalUint8(65537),'',empty]);
rejected([new OriginalUint8(32768),'',new OriginalUint8(32769)]);
assert.equal(bytes(call(new OriginalUint8(32768),'',new OriginalUint8(32768))).length,32768);
rejected([new OriginalUint8(65536),'',{}],TypeError);
rejected([new OriginalUint8(32768),'a'.repeat(32769),empty]);
rejected([new OriginalUint8(65536),'x',empty]);
rejected([empty,'€'.repeat(21846),empty]);
rejected([empty,'😀'.repeat(16385),empty]);
assert.equal(bytes(call(empty,'€'.repeat(21845)+'a',empty)).length,0);
assert.equal(bytes(call(empty,'😀'.repeat(16384),empty)).length,0);
rejected([empty,'\ud800',empty],TypeError);
rejected([empty,'\udc00',empty],TypeError);
rejected([empty,new String('text'),empty],TypeError);
for(const value of [new Uint16Array(1),new DataView(new ArrayBuffer(1)),new Proxy(empty,{}),new (class extends OriginalUint8 {})(1)]){
  rejected([value,'',empty],TypeError);
}
for(const value of [new Uint16Array([65535]),new Int8Array([-1])]){
  let tagHooks=0;
  Object.setPrototypeOf(value,OriginalUint8.prototype);
  Object.defineProperty(value,Symbol.toStringTag,{get(){tagHooks++;throw Error('caller tag getter')}});
  rejected([value,'',empty],TypeError);assert.equal(tagHooks,0);
}
const detached=new OriginalUint8([1]);structuredClone(detached.buffer,{transfer:[detached.buffer]});
rejected([detached,'',empty],TypeError);
const detachedEmpty=new OriginalUint8();structuredClone(detachedEmpty.buffer,{transfer:[detachedEmpty.buffer]});
rejected([detachedEmpty,'',empty],TypeError);
assert.equal(typeof SharedArrayBuffer,'function','shared input rejection requires SharedArrayBuffer');
const shared=new OriginalUint8(new SharedArrayBuffer(1));
assert.equal(shared.buffer instanceof SharedArrayBuffer,true);shared[0]=9;assert.equal(shared[0],9);
assert.equal(typeof ArrayBuffer.prototype.resize,'function','resizable input rejection requires resize');
const resizableBuffer=new ArrayBuffer(1,{maxByteLength:2}),resizable=new OriginalUint8(resizableBuffer);
assert.equal(resizableBuffer.resizable,true);assert.equal(resizableBuffer.maxByteLength,2);
resizableBuffer.resize(2);assert.equal(resizable.byteLength,2);resizableBuffer.resize(1);assert.equal(resizable.byteLength,1);
for(const value of [shared,resizable]){
  assert.deepEqual(bytes(call(new OriginalUint8([21]),'',empty)),new OriginalUint8([21]));
  rejected([value,'',empty],error=>error instanceof TypeError&&error.message==='argument 0 must be an ordinary attached fixed Uint8Array');
  assert.deepEqual(bytes(call(new OriginalUint8([23]),'',empty)),new OriginalUint8([23]),'rejection must preserve same-instance reuse');
}
const subclassBuffer=new (class extends ArrayBuffer {})(1);
rejected([new OriginalUint8(subclassBuffer),'',empty],TypeError);
let hooks=0;
const hooked=new OriginalUint8([9]);
Object.defineProperty(hooked.buffer,'constructor',{get(){hooks++;call(empty,'',empty);return ArrayBuffer}});
rejected([hooked,'',empty],TypeError);assert.equal(hooks,0);
const hookedView=new OriginalUint8([9]);
Object.defineProperty(hookedView,'constructor',{get(){hooks++;throw Error('view constructor hook')}});
rejected([hookedView,'',empty],TypeError);assert.equal(hooks,0);
const species=new OriginalUint8([8]);
Object.defineProperty(species.buffer,'constructor',{value:{get [Symbol.species](){hooks++;return ArrayBuffer}}});
rejected([species,'',empty],TypeError);assert.equal(hooks,0);
// Overshadowed length/buffer accessors are never consulted by intrinsic checks.
const shadowed=new OriginalUint8([3]);
for(const key of ['byteLength','byteOffset','buffer',Symbol.toStringTag])Object.defineProperty(shadowed,key,{get(){hooks++;throw Error('caller getter')}});
assert.deepEqual(bytes(call(shadowed,'',empty)),new OriginalUint8([3]));assert.equal(hooks,0);
assert.deepEqual(bytes(call(new OriginalUint8([7]),'',empty)),new OriginalUint8([7]),'rejection must not poison later calls');
const mixed=api.functions['probe.mixed'];
if(profile.requireMixed)assert.equal(typeof mixed,'function');
if(mixed){
  const full=new OriginalUint8(65536);
  for(const invalid of [0,'0',null,{},-(1n<<63n)-1n,1n<<63n])rejected([full,'',invalid,true],TypeError,mixed);
  for(const invalid of [0,1,'true',null,{}])rejected([full,'',0n,invalid],TypeError,mixed);
  rejected([empty,'x'.repeat(65536),0n,{}],TypeError,mixed);
  for(const endpoint of [-(1n<<63n),(1n<<63n)-1n])assert.equal(bytes(mixed(full,'',endpoint,true)).length,65536);
  assert.deepEqual(bytes(mixed(empty,'scalar recovery',0n,false)),new TextEncoder().encode('scalar recovery'));
  if(profile.recordFields){
    assert.equal(mixed(empty,'',0n,true)[profile.recordFields.valid],true);
    assert.equal(mixed(empty,'',0n,false)[profile.recordFields.valid],false);
  }
}
if(profile.utf8){
  assert.equal(api.functions['probe.greeting'](),'hello\0世界');
  assert.deepEqual(bytes(call(new OriginalUint8([0xff,0xc3,0x28]),'',empty)),new OriginalUint8([0xff,0xc3,0x28]),'Bytes must not be UTF-8 decoded');
}
// Accepted module-size boundaries still fail the exact artifact digest, after
// one snapshot copy. Capacity-plus-one fails before copy or hashing.
if(profile.moduleBoundaries)for(const length of [16777215,16777216]){
  const module=new OriginalUint8(length);reset();
  await assert.rejects(instantiate(module),/artifact authentication failed/);
  assert.equal(copies,1,'admitted module copied exactly once');
}
const oversizedModule=new OriginalUint8(16777217);
reset();await assert.rejects(instantiate(oversizedModule),RangeError);
assert.deepEqual([allocations,copies,encodings],[0,0,0],'module bound before copy/hash');
if(profile.moduleBoundaries){
  let moduleHooks=0;
  const hookedModule=new OriginalUint8(moduleBytes);
  Object.defineProperty(hookedModule.buffer,'constructor',{get(){moduleHooks++;throw Error('module constructor hook')}});
  reset();await assert.rejects(instantiate(hookedModule),TypeError);
  assert.deepEqual([allocations,copies,encodings,moduleHooks],[0,0,0,0]);
  // All rejected invocations leave the original instance usable.
  assert.deepEqual(bytes(call(new OriginalUint8([13]),'',empty)),new OriginalUint8([13]));
}
globalThis.Uint8Array=OriginalUint8;typedPrototype.set=originalSet;TextEncoder.prototype.encode=originalEncode;
console.log('owned-input-admission-v8-ok');
