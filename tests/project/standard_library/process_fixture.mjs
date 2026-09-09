import fs from 'node:fs';
import assert from 'node:assert/strict';
const module=new WebAssembly.Module(fs.readFileSync(process.argv[2]));
const provider=environmentProvider(module);
let instance,calls=0,settles=0;
Object.assign(provider.imports.env,{
 spx_process_run_v1:(tool,argv,length,input,inputLength,timeout,maxout,maxerr,pointer)=>{
  assert.equal(tool,7n);assert.equal(length,13n);assert.equal(inputLength,1n);
  assert.equal(timeout,100n);assert.equal(maxout,2n);assert.equal(maxerr,2n);
  const expected=[2,0,0,0,1,0,0,0,255,0,0,0,0];
  expected.forEach((value,index)=>assert.equal(provider.imports.env.spx_bytes_get(argv,BigInt(index)),value));
  assert.equal(provider.imports.env.spx_bytes_get(input,0n),66);calls++;
  const bytes=new Uint8Array(34);bytes[0]=1;bytes.set([252,255,255,255,3],8);bytes[16]=1;bytes[24]=1;bytes[32]=65;bytes[33]=33;
  const owner=provider.imports.env.spx_bytes_zeroed(34n);
  bytes.forEach((value,index)=>provider.imports.env.spx_bytes_set(owner,BigInt(index),value));
  new DataView((instance.exports.memory??instance.exports.__spx_byte_memory).buffer).setBigInt64(pointer,owner,true);
  return 0;
 },spx_process_settle_v1:()=>{settles++;return 0;},
});
instance=new WebAssembly.Instance(module,provider.imports);provider.attach(instance);
for(let i=0;i<3;i++){assert.equal(instance.exports['COMMAND_SYMBOL'](),1);provider.settled();}
assert.equal(calls,3);assert.equal(settles,3);
