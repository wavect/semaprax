import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {resolve} from 'node:path';

const root=resolve(process.argv[2]),mod=await import(pathToFileURL(resolve(root,'semaprax.js')));
assert.deepEqual(Object.keys(mod),['instantiate']);
const bytes=Uint8Array.from(readFileSync(resolve(root,'app.wasm')));
const runtime=await mod.instantiate(bytes);
assert(Object.isFrozen(runtime));assert.deepEqual(Object.keys(runtime),['call']);
for(const key of ['memory','instance','imports','arena','functions'])assert.equal(runtime[key],undefined);
const success=(id,args,value)=>{
  const result=runtime.call(id,...args);
  assert(Object.isFrozen(result));assert.deepEqual(result,{kind:'success',value});
};
success('web.content',[],42n);
success('',[],17n);
success('web.bool',[false],false);success('web.bool',[true],true);
success('__proto__',[],11n);success('web."</script>λ',[],13n);
for(const [id,args,domain,code] of [
  ['web.divide',[0n],'semaprax.arithmetic.v1',4],
  ['web.required',[false],'semaprax.contract.v1',1],
]){
  const result=runtime.call(id,...args);
  assert(Object.isFrozen(result));assert.deepEqual(result,{kind:'failure',domain,code});
  success('web.content',[],42n);
}
const capacity=runtime.call('web.capacity',5000n);
assert(Object.isFrozen(capacity));
assert.deepEqual(capacity,{kind:'capacity',cause:'cumulative_bytes'});
success('web.capacity',[1n],1n);success('web.content',[],42n);
for(const action of [
  ()=>runtime.call('unknown'),()=>runtime.call('web.bool',0),
  ()=>runtime.call('web.divide',1),()=>runtime.call('web.divide',1n<<63n),
  ()=>runtime.call('web.content',1n),
]){assert.throws(action,TypeError);success('web.content',[],42n)}
const corrupted=new Uint8Array(bytes);corrupted[corrupted.length-1]^=1;
await assert.rejects(mod.instantiate(corrupted),/authentication/);
await assert.rejects(mod.instantiate(bytes.buffer),TypeError);
const later=await mod.instantiate(bytes);
assert.deepEqual(later.call('web.content'),{kind:'success',value:42n});
console.log('string-web-node-ok');
