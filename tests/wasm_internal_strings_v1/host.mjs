import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {instantiate} from './program.mjs';
const bytes=Uint8Array.from(readFileSync('program.wasm'));
const cases=JSON.parse(readFileSync('cases.json','utf8'));
const api=await instantiate(bytes);
assert(Object.isFrozen(api));assert.deepEqual(Object.keys(api),['call']);
for(const [id,expected] of cases){
  for(let repeat=0;repeat<3;repeat++){
    const outcome=api.call(id);assert(Object.isFrozen(outcome));
    const observed=outcome.kind==='success'?`ok|${outcome.value}`:`${outcome.domain}|${outcome.code}`;
    assert.equal(observed,expected);
    assert.deepEqual(api.call('case.scalar',41n),{kind:'success',value:42n});
  }
  process.stdout.write(`${id}|${expected}\n`);
}
assert.deepEqual(api.call('case.bool',true),{kind:'success',value:true});
assert.deepEqual(api.call('case.bool',false),{kind:'success',value:false});
let coerced=0;
for(const value of [0,1.5,'1',null,{valueOf(){coerced++;return 1n}},-(1n<<63n)-1n,1n<<63n])assert.throws(()=>api.call('case.scalar',value),TypeError);
assert.equal(coerced,0);
assert.throws(()=>api.call('case.bool',1),TypeError);
assert.throws(()=>api.call('missing'),TypeError);
assert.throws(()=>api.call('case.scalar'),TypeError);
assert.throws(()=>api.call('case.scalar',1n,2n),TypeError);
assert.deepEqual(api.call('case.scalar',41n),{kind:'success',value:42n});

const changed=Uint8Array.from(bytes);changed[changed.length-1]^=1;
await assert.rejects(instantiate(changed),/authentication/);
await assert.rejects(instantiate(bytes.subarray(1)),/length/);
await assert.rejects(instantiate({buffer:bytes.buffer}),/ordinary/);
await assert.rejects(instantiate(new Uint8Array(new SharedArrayBuffer(bytes.length))),/ordinary/);
const detached=Uint8Array.from(bytes);structuredClone(detached.buffer,{transfer:[detached.buffer]});
await assert.rejects(instantiate(detached),/attached|ordinary/);

// Simulate an engine/host fault below the safe facade. No production fault
// hooks or raw instance exports are added. Each facade must poison permanently.
const realInstantiate=WebAssembly.instantiate;
for(const fault of ['throw','mint-trap','status','reenter','stack','scratch','false-capacity']){
  let apiUnderTest,invocations=0,mintedBeforeTrap=0;
  WebAssembly.instantiate=async(module,imports)=>{
    let selectedImports=imports;
    if(fault==='mint-trap'){
      const original=imports['semaprax.internal-strings.v1'];
      selectedImports={'semaprax.internal-strings.v1':{...original,literal(...args){
        const handle=original.literal(...args);assert.notEqual(handle,0n);mintedBeforeTrap++;
        throw new WebAssembly.RuntimeError('injected trap after an initialized host owner');
      }}};
    }
    const real=await realInstantiate(module,selectedImports);
    const exports={...real.exports};
    for(const [name,value] of Object.entries(exports))if(name.startsWith('__spx_call_')){
      exports[name]=(...args)=>{
        invocations++;
        if(fault==='throw')throw new Error('injected host exception');
        if(fault==='status')return 12;
        if(fault==='false-capacity')return 11;
        if(fault==='reenter'){try{apiUnderTest.call('case.scalar',1n)}catch{}}
        const status=value(...args);
        if(fault==='stack')exports.__spx_stack_pointer.value=0;
        if(fault==='scratch'){new DataView(exports.memory.buffer).setBigUint64(65536,1n,true);return 9}
        return status;
      };
    }
    return {exports};
  };
  try{apiUnderTest=await instantiate(bytes)}finally{WebAssembly.instantiate=realInstantiate}
  assert.throws(()=>fault==='mint-trap'?apiUnderTest.call('case.content'):apiUnderTest.call('case.scalar',41n),/poisoned/);
  if(fault==='mint-trap')assert.equal(mintedBeforeTrap,1);
  assert.equal(invocations,1);
  assert.throws(()=>apiUnderTest.call('case.scalar',41n),/poisoned/);
  assert.equal(invocations,1,'poisoned facade invoked engine again');
}
