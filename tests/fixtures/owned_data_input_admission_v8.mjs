import fs from 'node:fs';
import assert from 'node:assert/strict';

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
function rejected(args,kind=RangeError){
  reset();assert.throws(()=>call(...args),kind);
  assert.deepEqual([allocations,copies,encodings],[0,0,0],'reject before any payload snapshot');
}
function bytes(value){
  if(value instanceof OriginalUint8)return value;
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
const detached=new OriginalUint8([1]);structuredClone(detached.buffer,{transfer:[detached.buffer]});
rejected([detached,'',empty],TypeError);
const detachedEmpty=new OriginalUint8();structuredClone(detachedEmpty.buffer,{transfer:[detachedEmpty.buffer]});
rejected([detachedEmpty,'',empty],TypeError);
if(typeof SharedArrayBuffer!=='undefined')rejected([new OriginalUint8(new SharedArrayBuffer(1)),'',empty],TypeError);
const resizable=new ArrayBuffer(1,{maxByteLength:2});
if(resizable.resizable)rejected([new OriginalUint8(resizable),'',empty],TypeError);
const subclassBuffer=new (class extends ArrayBuffer {})(1);
rejected([new OriginalUint8(subclassBuffer),'',empty],TypeError);
let hooks=0;
const hooked=new OriginalUint8([9]);
Object.defineProperty(hooked.buffer,'constructor',{get(){hooks++;call(empty,'',empty);return ArrayBuffer}});
rejected([hooked,'',empty],TypeError);assert.equal(hooks,0);
const species=new OriginalUint8([8]);
Object.defineProperty(species.buffer,'constructor',{value:{get [Symbol.species](){hooks++;return ArrayBuffer}}});
rejected([species,'',empty],TypeError);assert.equal(hooks,0);
// Overshadowed length/buffer accessors are never consulted by intrinsic checks.
const shadowed=new OriginalUint8([3]);
for(const key of ['byteLength','byteOffset','buffer'])Object.defineProperty(shadowed,key,{get(){hooks++;throw Error('caller getter')}});
assert.deepEqual(bytes(call(shadowed,'',empty)),new OriginalUint8([3]));assert.equal(hooks,0);
assert.deepEqual(bytes(call(new OriginalUint8([7]),'',empty)),new OriginalUint8([7]),'rejection must not poison later calls');
const oversizedModule=new OriginalUint8(16777217);
reset();await assert.rejects(instantiate(oversizedModule),RangeError);
assert.deepEqual([allocations,copies,encodings],[0,0,0],'module bound before copy/hash');
globalThis.Uint8Array=OriginalUint8;typedPrototype.set=originalSet;TextEncoder.prototype.encode=originalEncode;
console.log('owned-input-admission-v8-ok');
